// The scripts in config/vasttweaks/, compiled into this load's data.
fn vasttweaks_data_pack_files(game: &crate::game::Game<'_>, files: &mut Vec<(String, Vec<u8>)>) {
    let _ = game;
    crate::game::vasttweaks_client::data_pack_files(files);
}
