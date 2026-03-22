#[allow(clippy::all, clippy::pedantic, clippy::absolute_paths)]
mod inner {
    tonic::include_proto!("ryanseipp.identity.v1");
}
pub use inner::*;

pub mod email {
    pub mod v1 {
        #[allow(clippy::all, clippy::pedantic, clippy::absolute_paths)]
        mod inner {
            tonic::include_proto!("ryanseipp.email.v1");
        }
        pub use inner::*;
    }
}

pub mod events {
    #[allow(clippy::all, clippy::pedantic, clippy::absolute_paths)]
    mod inner {
        tonic::include_proto!("ryanseipp.events.v1");
    }
    pub use inner::*;
}
