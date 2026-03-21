pub mod identity {
    pub mod v1 {
        #[allow(clippy::absolute_paths)]
        mod inner {
            tonic::include_proto!("ryanseipp.identity.v1");
        }
        pub use inner::*;
    }
}
