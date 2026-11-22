//! Persist completed startup checks immediately through the existing atomic store.
use crate::{input::Controller, settings::Store};

pub(super) fn persist(c: &mut Controller, store: &mut Option<Store>) -> Result<(), String> {
    if !c.compound_check_save_pending {
        return Ok(());
    }
    let store = store
        .as_mut()
        .ok_or("compound extension check completed but no user config directory is available")?;
    store.settings = c.preferences.clone();
    store.save()?;
    c.compound_check_save_pending = false;
    c.preferences_changed = false;
    Ok(())
}

#[cfg(test)]
mod tests;
