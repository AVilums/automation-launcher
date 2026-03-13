pub mod browse;
pub mod cache;
pub mod favorites;
pub mod settings;

#[derive(PartialEq, Clone, Copy)]
pub enum Tab {
    Browse,
    Favorites,
    Cache,
    Settings,
}
