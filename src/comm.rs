//! # Communications Library
//!
//! The method used to communicate between the child process and the parent
//! process uses the simple STDIN, STDOUT, and STDERR.  The top-level README
//! contains details about this communication method.

pub mod event;
pub mod packet;
pub mod sizedpacket;
pub mod splitter;

mod rwutil;
