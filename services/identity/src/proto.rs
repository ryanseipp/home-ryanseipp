tonic::include_proto!("ryanseipp.identity.v1");

pub mod email {
    pub mod v1 {
        tonic::include_proto!("ryanseipp.email.v1");
    }
}

pub mod events {
    tonic::include_proto!("ryanseipp.events.v1");
}
