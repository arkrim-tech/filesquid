//! The desktop's file and folder import (Task DE-1.2; F-002, DE-S-008, MD-S-013, ADR 038):
//! the facts the operating system provides about each source file, gathered before its bytes
//! are read ([`facts`], [`volume`], [`host`], [`xattr`]), and the name form of its original
//! file name ([`names`]).

pub mod facts;
pub mod file;
pub mod host;
pub mod names;
pub mod time;
pub mod volume;
pub mod xattr;
