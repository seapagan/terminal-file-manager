use super::*;

impl<F: FnMut(Event) -> Decision> Worker<'_, F> {
    pub(super) fn directory(
        &mut self,
        source: &Entry,
        destination: &Path,
        previous: Option<&Entry>,
        depth: usize,
        result: &mut EntryResult,
    ) -> Result<()> {
        if previous.is_some_and(|entry| !entry.may_recurse()) {
            return Err(crate::operations::plan::invalid(
                "Atomic file/directory replacement unsupported; choose Keep both or Skip",
            )
            .into());
        }
        let source_parents = Parents::capture(&source.path)?;
        let target_parents = Parents::capture(destination)?;
        safety::destination_matches(destination, previous)?;
        if previous.is_none() {
            create_directory(destination)?;
            result.created_directory = true;
        }
        let mut target = crate::fs::inspect(destination);
        let guards = [source_parents, target_parents];
        result.recovery = Some(Box::new(crate::operations::Recovery::directory(
            source, &target, &guards,
        )));
        let complete = self.children(source, &target, depth, [&guards[0], &guards[1]])?;
        if !complete {
            return Err(safety::changed(
                "Child transfer incomplete; directory finalisation deferred",
            ));
        }
        let finished = self.finish_directory(
            source,
            &mut target,
            [&guards[0], &guards[1]],
            result.created_directory,
        );
        if let Some(evidence) = &mut result.recovery {
            evidence.committed = Some(target);
        }
        finished
    }

    fn children(
        &mut self,
        source: &Entry,
        target: &Entry,
        depth: usize,
        guards: [&Parents; 2],
    ) -> Result<bool> {
        let mut complete = true;
        let entries: std::io::Result<Vec<_>> = fs::read_dir(&source.path)?.collect();
        for item in entries? {
            self.check_cancel()?;
            guards[0].validate()?;
            safety::revalidate(source, false)?;
            guards[1].validate()?;
            safety::revalidate(target, false)?;
            let entry = crate::fs::inspect(&item.path());
            complete &= self.entry(&entry, Some(&target.path.join(&entry.name)), depth + 1);
            if self.record.unresolved() {
                return Err(Failure {
                    outcome: Outcome::OutcomeUnknown,
                    detail: "Child outcome unresolved".into(),
                });
            }
        }
        Ok(complete)
    }

    fn finish_directory(
        &mut self,
        source: &Entry,
        target: &mut Entry,
        guards: [&Parents; 2],
        created: bool,
    ) -> Result<()> {
        self.check_cancel()?;
        guards[1].validate()?;
        safety::revalidate(target, false)?;
        finish_permissions(source, target, created, guards[0])?;
        if self.plan.operation == Operation::Move {
            self.remove_directory(source, target, guards)?;
        }
        Ok(())
    }

    fn remove_directory(
        &mut self,
        source: &Entry,
        target: &Entry,
        guards: [&Parents; 2],
    ) -> Result<()> {
        (self.notify)(Event::Progress {
            source: source.path.clone(),
            destination: target.path.clone(),
            stage: Stage::Removing,
            bytes: 0,
        });
        self.check_cancel()?;
        guards[0].validate()?;
        guards[1].validate()?;
        safety::revalidate(target, false)?;
        safety::revalidate(source, false)?;
        safety::acknowledged_remove(source, |path| fs::remove_dir(path))?;
        Ok(())
    }
}

fn create_directory(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(path)
    }
    #[cfg(not(unix))]
    fs::create_dir(path)
}

fn finish_permissions(
    source: &Entry,
    target: &mut Entry,
    created: bool,
    source_parents: &Parents,
) -> Result<()> {
    if created {
        source_parents.validate()?;
        safety::revalidate(source, false)?;
        let file = crate::operations::mutations::open_attributes(target)?;
        safety::opened_object(target, &file)?;
        crate::operations::mutations::set_attributes(&file, directory_permissions(source, &file)?)?;
        let mut expected = target.clone();
        expected.attributes = permission_attributes(source, target)?;
        safety::revalidate(&expected, false)?;
        *target = expected;
    }
    Ok(())
}

fn directory_permissions(source: &Entry, _file: &fs::File) -> Result<fs::Permissions> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = source.mode.ok_or_else(|| {
            crate::operations::plan::invalid("Source directory permissions unavailable")
        })?;
        Ok(fs::Permissions::from_mode(mode))
    }
    #[cfg(windows)]
    {
        let mut permissions = _file.metadata()?.permissions();
        let flags = u32::from_str_radix(source.attributes.trim_start_matches("0x"), 16)
            .map_err(std::io::Error::other)?;
        permissions.set_readonly(flags & 1 != 0);
        Ok(permissions)
    }
}

fn permission_attributes(source: &Entry, _target: &Entry) -> Result<String> {
    #[cfg(unix)]
    {
        let mode = source.mode.ok_or_else(|| {
            crate::operations::plan::invalid("Source directory permissions unavailable")
        })?;
        Ok(crate::permissions::mode_to_octal(mode))
    }
    #[cfg(windows)]
    {
        let flags = |entry: &Entry| {
            u32::from_str_radix(entry.attributes.trim_start_matches("0x"), 16)
                .map_err(std::io::Error::other)
        };
        Ok(format!(
            "0x{:08x}",
            (flags(_target)? & !1) | (flags(source)? & 1)
        ))
    }
}

pub(super) fn failure(result: &EntryResult, mut error: Failure, unknown_children: bool) -> Failure {
    if result.kind == EntryKind::Directory
        && let Some(evidence) = &result.recovery
    {
        error.outcome = if unknown_children
            || error.outcome == Outcome::OutcomeUnknown
            || evidence.validate_directory().is_err()
        {
            Outcome::OutcomeUnknown
        } else {
            Outcome::CopiedSourceRetained
        };
    }
    error
}
