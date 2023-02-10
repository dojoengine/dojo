use std::str::FromStr;

use apibara_client_protos::pb::{
    starknet::v1alpha2::{EventFilter, FieldElement, Filter, HeaderFilter},
    stream::v1alpha2::{
        stream_client::StreamClient, stream_data_response::Message, Cursor, Data,
        StreamDataRequest, StreamDataResponse,
    },
};
use futures_util::Stream;
use log::debug;
use tokio::sync::mpsc;
use tonic::transport::{Channel, Endpoint};

pub enum Chain {
    AlphaMainnet,
    AlphaGoerli,
    AlphaGoerli2,
}

impl From<Chain> for &'static str {
    fn from(chain: Chain) -> Self {
        match chain {
            Chain::AlphaMainnet => "https://mainnet.starknet.a5a.ch:443",
            Chain::AlphaGoerli => "https://goerli.starknet.a5a.ch:443",
            Chain::AlphaGoerli2 => "https://goerli2.starknet.a5a.ch:443",
        }
    }
}

pub struct ApibaraClient {
    stream: StreamClient<Channel>,
    stream_id: u64,
    response_stream: Option<tonic::codec::Streaming<StreamDataResponse>>,
    sender: Option<mpsc::Sender<StreamDataRequest>>,
}

impl ApibaraClient {
    pub async fn new(path: &String) -> Result<Self, tonic::transport::Error> {
        println!("{}", path);
        let channel = Endpoint::from_str(path)?.connect().await?;

        debug!("Connected to Apibara server {:?}", channel);
        let stream = StreamClient::new(channel);
        Ok(Self { stream, stream_id: 0, response_stream: None, sender: None })
    }

    pub async fn request_data<'x>(
        &'x mut self,
        filter: Filter,
    ) -> Result<impl Stream<Item = Result<Option<Data>, tonic::Status>> + 'x, tonic::Status> {
        self.stream_id += 1;
        let request = StreamDataRequest {
            stream_id: Some(self.stream_id),
            batch_size: Some(1),
            starting_cursor: Some(Cursor { order_key: 1, unique_key: vec![] }),
            finality: None,
            filter: Some(filter.into()),
        };

        let (sender, receiver) = mpsc::channel::<StreamDataRequest>(1);
        let str = tokio_stream::wrappers::ReceiverStream::new(receiver);
        sender.send(request).await;
        self.sender = Some(sender);
        self.response_stream = Some(self.stream.stream_data(str).await?.into_inner());

        Ok(futures::stream::unfold(self, |s| async {
            let message = s.response_stream.as_mut().unwrap().message().await;
            match message {
                Ok(Some(mess)) => {
                    if mess.stream_id != s.stream_id {
                        return Some((Ok(None), s));
                    }
                    match mess.message {
                        Some(Message::Data(a)) => Some((Ok(Some(a)), s)),
                        _ => Some((Ok(None), s)),
                    }
                }
                Ok(None) => {
                    debug!("Stopping");
                    None
                }
                Err(e) => {
                    debug!("Error receiving message {:?}", e);
                    Some((Err(e), s))
                }
            }
        }))
    }
}
