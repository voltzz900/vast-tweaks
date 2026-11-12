// The mod's row in Options > Mods.
fn vasttweaks_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: "vasttweaks".into(),
        title: "Vast Tweaks 1.1.0".into(),
        description: "Recipe, tag and unify scripts from config/vasttweaks/*.vt, loaded with each world and on /reload.".into(),
        keys: Vec::new(),
    });
}
