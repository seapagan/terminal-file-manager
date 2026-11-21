use super::Receipt;
use crate::operations::{
    Outcome,
    safety::{Failure, Result},
};

#[derive(Clone, Debug)]
pub(super) struct Acknowledgement {
    pub status: i32,
    pub recycled: bool,
    pub metadata: super::ShellMetadata,
    pub error: Option<String>,
    pub parsing_name: Option<Vec<u16>>,
}

pub(super) fn classify(
    performed: bool,
    aborted: Option<bool>,
    callbacks: &[Acknowledgement],
    retained: bool,
    absent: bool,
) -> Result<Option<Receipt>> {
    // The configured Shell warning owns consent. Only verified completion may
    // become irreversible success; missing/contradictory evidence stays critical.
    if let [ack] = callbacks
        && performed
        && aborted == Some(false)
        && absent
        && !retained
        && permanent_acknowledged(ack)
    {
        return Ok(None);
    }
    if permanent_deletion(callbacks) {
        return Err(Failure { outcome: Outcome::SafetyIncident,
            detail: "CRITICAL SAFETY INCIDENT: Windows reported permanent deletion during F8 Trash; stop and report this native integration failure".into() });
    }
    if let [ack] = callbacks {
        if performed && aborted == Some(false) && recycle_acknowledged(ack) && !retained {
            return Ok(Some(Receipt::Windows {
                parsing_name: ack.parsing_name.clone(),
                metadata: ack.metadata.clone(),
            }));
        }
        if let Some(failure) = retained_refusal(ack, retained) {
            return Err(failure);
        }
    }
    let errors = callbacks
        .iter()
        .map(|ack| {
            format!(
                "callback status 0x{:08X}: {}",
                ack.status as u32,
                ack.error.as_deref().unwrap_or("no native error")
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    Err(Failure {
        outcome: Outcome::OutcomeUnknown,
        detail: format!(
            "Native Trash acknowledgement missing, interrupted or contradictory; reconciliation required, no automatic retry. {errors}"
        ),
    })
}

fn retained_refusal(ack: &Acknowledgement, retained: bool) -> Option<Failure> {
    match (retained, ack.recycled, ack.status) {
        // COPYENGINE_S_USER_IGNORED means No, not a completed deletion.
        (true, false, 0x0027_0005) => Some(Failure {
            outcome: Outcome::Skipped,
            detail: "Windows recycling skipped; approved source retained".into(),
        }),
        (true, false, status) if status == 0x8027_0000_u32 as i32 => Some(Failure {
            outcome: Outcome::Cancelled,
            detail: "Windows recycling cancelled; approved source identity remains intact".into(),
        }),
        (_, _, status) if status == 0x8027_0000_u32 as i32 => None,
        (true, _, status) if status < 0 => Some(Failure {
            outcome: Outcome::Failed,
            detail: format!(
                "Windows refused recycling; approved source identity remains intact: {}",
                ack.error.as_deref().unwrap_or("no error details")
            ),
        }),
        _ => None,
    }
}

fn recycle_acknowledged(ack: &Acknowledgement) -> bool {
    // Shell retry, ignored, pending and pending-delete statuses are control flow.
    let control = matches!(
        ack.status,
        0x0027_0004 | 0x0027_0005 | 0x0027_000B | 0x0027_0010
    );
    ack.status >= 0 && ack.recycled && !control
}

fn permanent_acknowledged(ack: &Acknowledgement) -> bool {
    matches!(ack.status, 0 | 0x0027_0008)
        && !ack.recycled
        && ack.parsing_name.is_none()
        && ack.error.is_none()
}

fn permanent_deletion(callbacks: &[Acknowledgement]) -> bool {
    // COPYENGINE_S_DONT_PROCESS_CHILDREN also completes an atomic native delete.
    // These statuses without a receipt require verified completion above.
    callbacks
        .iter()
        .any(|ack| matches!(ack.status, 0 | 0x0027_0008) && !ack.recycled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ack(succeeded: bool, recycled: bool) -> Acknowledgement {
        Acknowledgement {
            status: if succeeded { 0 } else { i32::MIN },
            recycled,
            metadata: super::super::ShellMetadata::default(),
            error: Some("native refusal".into()),
            parsing_name: None,
        }
    }

    #[test]
    fn positive_shell_control_statuses_never_claim_permanent_deletion() {
        for status in [0x0027_0005, 0x0027_0004, 0x0027_000B, 0x0027_0010] {
            let mut ignored = ack(true, false);
            ignored.status = status;
            for aborted in [Some(false), Some(true), None] {
                let expected = if status == 0x0027_0005 {
                    Outcome::Skipped
                } else {
                    Outcome::OutcomeUnknown
                };
                assert_eq!(
                    classify(true, aborted, &[ignored.clone()], true, false)
                        .unwrap_err()
                        .outcome,
                    expected
                );
                assert_eq!(
                    classify(true, aborted, &[ignored.clone()], false, false)
                        .unwrap_err()
                        .outcome,
                    Outcome::OutcomeUnknown
                );
            }
        }
    }

    #[test]
    fn callback_not_optional_properties_proves_recycling() {
        let receipt = classify(true, Some(false), &[ack(true, true)], false, false).unwrap();
        assert!(matches!(
            receipt,
            Some(Receipt::Windows {
                parsing_name: None,
                ..
            })
        ));
        assert!(receipt.unwrap().path().is_none());
    }

    #[test]
    fn verified_completed_permanent_deletion_is_successful_without_a_receipt() {
        for status in [0, 0x0027_0008] {
            let mut deleted = ack(true, false);
            deleted.status = status;
            deleted.error = None;
            assert!(
                classify(true, Some(false), &[deleted], false, true)
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn unverified_permanent_deletion_is_critical_even_with_abort_or_rpc_failure() {
        for performed in [true, false] {
            for aborted in [Some(false), Some(true), None] {
                assert_eq!(
                    classify(performed, aborted, &[ack(true, false)], false, false)
                        .unwrap_err()
                        .outcome,
                    Outcome::SafetyIncident
                );
            }
        }
    }

    #[test]
    fn unverified_nonzero_deletion_without_a_receipt_is_critical() {
        let mut deleted = ack(true, false);
        deleted.status = 0x0027_0008;
        for retained in [true, false] {
            for performed in [true, false] {
                for aborted in [Some(false), Some(true), None] {
                    assert_eq!(
                        classify(performed, aborted, &[deleted.clone()], retained, false)
                            .unwrap_err()
                            .outcome,
                        Outcome::SafetyIncident
                    );
                }
            }
        }
    }

    #[test]
    fn permanent_success_requires_uncontradicted_terminal_evidence() {
        let mut deleted = ack(true, false);
        deleted.error = None;
        for status in [0, 0x0027_0008] {
            deleted.status = status;
            for (performed, aborted, retained, absent) in [
                (false, Some(false), false, true),
                (true, Some(true), false, true),
                (true, None, false, true),
                (true, Some(false), true, true),
                (true, Some(false), false, false),
            ] {
                assert_eq!(
                    classify(performed, aborted, &[deleted.clone()], retained, absent)
                        .unwrap_err()
                        .outcome,
                    Outcome::SafetyIncident
                );
            }
            assert_eq!(
                classify(
                    true,
                    Some(false),
                    &[deleted.clone(), deleted.clone()],
                    false,
                    true
                )
                .unwrap_err()
                .outcome,
                Outcome::SafetyIncident
            );
            let mut contradictory = deleted.clone();
            contradictory.parsing_name = Some(vec![1]);
            assert_eq!(
                classify(true, Some(false), &[contradictory], false, true)
                    .unwrap_err()
                    .outcome,
                Outcome::SafetyIncident
            );
        }
    }

    #[test]
    fn native_user_cancellation_requires_the_original_source_identity() {
        let mut cancelled = ack(false, false);
        cancelled.status = 0x8027_0000_u32 as i32;
        for performed in [true, false] {
            for aborted in [Some(false), Some(true), None] {
                assert_eq!(
                    classify(performed, aborted, &[cancelled.clone()], true, false)
                        .unwrap_err()
                        .outcome,
                    Outcome::Cancelled
                );
                assert_eq!(
                    classify(performed, aborted, &[cancelled.clone()], false, false)
                        .unwrap_err()
                        .outcome,
                    Outcome::OutcomeUnknown
                );
            }
        }
    }

    #[test]
    fn native_cancellation_with_a_recycle_receipt_remains_unknown() {
        let mut cancelled = ack(false, true);
        cancelled.status = 0x8027_0000_u32 as i32;
        for retained in [true, false] {
            for performed in [true, false] {
                for aborted in [Some(false), Some(true), None] {
                    assert_eq!(
                        classify(performed, aborted, &[cancelled.clone()], retained, false)
                            .unwrap_err()
                            .outcome,
                        Outcome::OutcomeUnknown
                    );
                }
            }
        }
    }

    #[test]
    fn missing_contradictory_failed_and_interrupted_acknowledgements_remain_unknown() {
        for (performed, aborted, callbacks) in [
            (true, Some(false), vec![]),
            (true, Some(true), vec![ack(true, true)]),
            (false, Some(false), vec![ack(true, true)]),
            (true, None, vec![ack(true, true)]),
            (true, Some(false), vec![ack(true, true), ack(true, true)]),
            (true, Some(false), vec![ack(false, false)]),
        ] {
            assert_eq!(
                classify(performed, aborted, &callbacks, false, false)
                    .unwrap_err()
                    .outcome,
                Outcome::OutcomeUnknown
            );
        }
        assert_eq!(
            classify(true, Some(false), &[ack(true, true)], true, false)
                .unwrap_err()
                .outcome,
            Outcome::OutcomeUnknown
        );
        assert_eq!(
            classify(true, Some(false), &[ack(false, false)], true, false)
                .unwrap_err()
                .outcome,
            Outcome::Failed
        );
    }
}
