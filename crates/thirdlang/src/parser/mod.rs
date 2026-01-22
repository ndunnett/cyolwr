#[cfg(feature = "pest_parser")]
mod pest_impl;

#[cfg(feature = "pest_parser")]
pub use pest_impl::parse;

#[cfg(feature = "winnow_parser")]
mod winnow_impl;

#[cfg(feature = "winnow_parser")]
pub use winnow_impl::parse;

const _: () = {
    #[cfg(not(any(feature = "pest_parser", feature = "winnow_parser")))]
    panic!("no parsing feature selected")
};
