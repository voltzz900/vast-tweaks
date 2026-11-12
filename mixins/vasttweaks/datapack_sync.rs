// With scripts in play, the clients get the server's recipes of the game's own types.
fn vasttweaks_datapack_sync(game: &mut crate::game::Game<'_>, recipe_types: &mut Vec<String>) {
    let _ = game;
    crate::game::vasttweaks_client::datapack_sync(recipe_types);
}
