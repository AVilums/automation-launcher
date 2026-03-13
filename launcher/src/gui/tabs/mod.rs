pub mod browse;
pub mod cache;
pub mod favorites;
pub mod settings;
pub mod systems;

#[derive(PartialEq, Clone, Copy)]
pub enum Tab {
    Browse,
    Favorites,
    Cache,
    Systems,
    Settings,
}
