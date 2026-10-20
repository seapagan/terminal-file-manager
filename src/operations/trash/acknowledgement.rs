use super::Receipt;
use crate::operations::{
    Outcome,
    safety::{Failure, Result},
};

#[derive(Clone, Debug)]
pub(super) struct Acknowledgement {
    pub succeeded: bool,
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
) -> Result<Receipt> {
    if permanent_deletion(callbacks) {
        return Err(Failure { outcome: Outcome::SafetyIncident,
            detail: "CRITICAL SAFETY INCIDENT: Windows reported permanent deletion during F8 Trash; stop and report this native integration failure".into() });
    }
    if let [ack] = callbacks {
        if performed && aborted == Some(false) && ack.succeeded && ack.recycled && !retained {
            return Ok(Receipt::Windows {
                parsing_name: ack.parsing_name.clone(),
                metadata: ack.metadata.clone(),
            });
        }
        if !ack.succeeded && retained {
            return Err(Failure {
                outcome: Outcome::Failed,
                detail: format!(
                    "Windows refused recycling; approved source identity remains intact: {}",
                    ack.error.as_deref().unwrap_or("no error details")
                ),
            });
        }
    }
    let errors = callbacks
        .iter()
        .filter_map(|ack| ack.error.as_deref())
        .collect::<Vec<_>>()
        .join("; ");
    Err(Failure {
        outcome: Outcome::OutcomeUnknown,
        detail: format!(
            "Native Trash acknowledgement missing, interrupted or contradictory; reconciliation required, no automatic retry. {errors}"
        ),
    })
}

fn permanent_deletion(callbacks: &[Acknowledgement]) -> bool {
    callbacks.iter().any(|ack| ack.succeeded && !ack.recycled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ack(succeeded: bool, recycled: bool) -> Acknowledgement {
        Acknowledgement {
            succeeded,
            recycled,
            metadata: super::super::ShellMetadata::default(),
            error: Some("native refusal".into()),
            parsing_name: None,
        }
    }

    #[test]
    fn callback_not_optional_properties_proves_recycling() {
        let receipt = classify(true, Some(false), &[ack(true, true)], false).unwrap();
        assert!(matches!(
            receipt,
            Receipt::Windows {
                parsing_name: None,
                ..
            }
        ));
        assert!(receipt.path().is_none());
    }

    #[test]
    fn permanent_deletion_is_critical_even_with_abort_or_rpc_failure() {
        for performed in [true, false] {
            for aborted in [Some(false), Some(true), None] {
                assert_eq!(
                    classify(performed, aborted, &[ack(true, false)], false)
                        .unwrap_err()
                        .outcome,
                    Outcome::SafetyIncident
                );
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
                classify(performed, aborted, &callbacks, false)
                    .unwrap_err()
                    .outcome,
                Outcome::OutcomeUnknown
            );
        }
        assert_eq!(
            classify(true, Some(false), &[ack(true, true)], true)
                .unwrap_err()
                .outcome,
            Outcome::OutcomeUnknown
        );
        assert_eq!(
            classify(true, Some(false), &[ack(false, false)], true)
                .unwrap_err()
                .outcome,
            Outcome::Failed
        );
    }
}
