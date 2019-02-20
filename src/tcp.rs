use mio::tcp::{TcpListener as MioTcpListener, TcpStream as MioTcpStream};
use futures::{Poll, stream::Stream, task::LocalWaker};
use std::net::addr::SocketAddr;
use std::pin::Pin;

struct TcpListener;

impl Stream for TcpListener {
    type item = (TcpStream, SocketAddr);
    fn poll_next(
        self: Pin<&mut Self>,
        lw: &LocalWaker) -> Poll<Option<Self::Item>> {
        panic!();
    }
}

