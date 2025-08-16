mod error;
mod request;
mod runner_standard;

pub use error::*;
pub use request::*;
pub use runner_standard::*;

pub trait Sender {
    // send sends the request to the LSP and returns the RequestID for the request
    // This enables the application to continue after the request has been made instead
    // of blocking. If you want to block use the provided `request`
    fn send<R: Into<Request>>(&mut self, msg: R) -> Result<RequestID>;
}
// Runner defines the required functions to interact with an LSP
pub trait Runner {
    // try_response will try to get the response from the endpoint, if it can't
    // it must return a NotReady error to let the caller know we aren't ready yet
    fn try_response<R: Into<RequestID>>(req_id: R) -> Result<Response>;

    // response is blocking, it will wait until a response has been received before
    // returning
    fn response<R: Into<RequestID>>(req_id: R) -> Result<Response>;
}

pub trait LspRequest {}

pub struct Response {}

pub type RequestID = u32;
