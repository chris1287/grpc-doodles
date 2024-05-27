use tonic::{
    async_trait,
    Request,
    Response,
    Status,
    Streaming,
    transport::Server
};
use tokio::{
    net::UnixListener,
    sync::mpsc
};
use tokio_stream::{wrappers::UnixListenerStream, wrappers::ReceiverStream, Stream};
use std::pin::Pin;

pub mod foo {
    tonic::include_proto!("foo");
}

pub struct MyFooServiceImpl {
}

#[async_trait]
impl foo::foo_service_server::FooService for MyFooServiceImpl {
    type ComputeStream = Pin<Box<dyn Stream<Item = Result<foo::Output, Status>> + Send>>;

    async fn compute(
        &self,
        in_stream: Request<Streaming<foo::Input>>,
    ) -> Result<Response<Self::ComputeStream>, Status> {
        println!("new incoming stream: {:?}", in_stream);
        let (tx, rx) = mpsc::channel(1);
        let output_stream = ReceiverStream::new(rx);
        tokio::spawn(handle_compute(tx.clone(), in_stream.into_inner()));
        Ok(Response::new(
            Box::pin(output_stream)
        ))
    }
}

async fn handle_compute(tx: mpsc::Sender<Result<foo::Output, Status>>, mut in_stream: Streaming<foo::Input>) {
    println!("ready to compute");
    loop {
        match in_stream.message().await {
            Ok(Some(input)) => {
                println!("received input: {:?}", input);
                let output = foo::Output {
                    input: Some(input.clone()),
                    res: input.a + input.b,
                };
                tx.send(Ok(output)).await.unwrap();
            }
            Ok(None) => {
                println!("end of stream");
                break;
            }
            Err(e) => {
                println!("error: {:?}", e);
                break;
            }
        }
    }
}

pub async fn run_server(grpc_socket_path: std::path::PathBuf) {
    let _ = std::fs::remove_file(&grpc_socket_path);
    let uds = UnixListener::bind(grpc_socket_path).unwrap();
    let uds_stream = UnixListenerStream::new(uds);

    let svc = foo::foo_service_server::FooServiceServer::new(MyFooServiceImpl {});

    Server::builder()
        .add_service(svc)
        .serve_with_incoming(uds_stream)
        .await
        .unwrap();
}

#[tokio::main]
async fn main() {
    let grpc_socket_path = std::path::PathBuf::from("/tmp/foo.sock");
    run_server(grpc_socket_path).await;
}
