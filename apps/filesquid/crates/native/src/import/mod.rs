//! The desktop's file and folder import (Tasks DE-1.2, DE-1.7; F-002, DE-S-008, DE-S-019,
//! MD-S-013, ADR 038, ADR 041): the facts the operating system provides about each source
//! file, gathered before its bytes are read ([`facts`], [`volume`], [`host`], [`xattr`]), the
//! name form of its original file name ([`names`]), the durable job ([`job`], [`plans`],
//! [`worker`]) and the library index it registers each file in ([`index`]).

pub mod facts;
pub mod file;
pub mod host;
pub mod index;
pub mod job;
pub mod names;
pub mod placing;
pub mod plans;
pub mod source;
pub mod time;
pub mod volume;
pub mod worker;
pub mod xattr;
