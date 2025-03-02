use std::io;

use distant_auth::msg::{Authentication, AuthenticationResponse};
use distant_auth::AuthHandler;
use log::*;

use crate::client::Client;
use crate::common::{ConnectionId, Destination, Map, Request};
use crate::manager::data::{
    ConnectionInfo, ConnectionList, ManagerCapabilities, ManagerRequest, ManagerResponse,
};

mod channel;
pub use channel::*;

/// Represents a client that can connect to a remote server manager.
pub type ManagerClient = Client<ManagerRequest, ManagerResponse>;

impl ManagerClient {
    /// Request that the manager launches a new server at the given `destination` with `options`
    /// being passed for destination-specific details, returning the new `destination` of the
    /// spawned server.
    ///
    ///  The provided `handler` will be used for any authentication requirements when connecting to
    ///  the remote machine to spawn the server.
    pub async fn launch(
        &mut self,
        destination: impl Into<Destination>,
        options: impl Into<Map>,
        mut handler: impl AuthHandler + Send,
    ) -> io::Result<Destination> {
        let destination = Box::new(destination.into());
        let options = options.into();
        trace!("launch({}, {})", destination, options);

        let mut mailbox = self
            .mail(ManagerRequest::Launch {
                destination: destination.clone(),
                options,
            })
            .await?;

        // Continue to process authentication challenges and other details until we are either
        // launched or fail
        while let Some(res) = mailbox.next().await {
            match res.payload {
                ManagerResponse::Authenticate { id, msg } => match msg {
                    Authentication::Initialization(x) => {
                        if log::log_enabled!(Level::Debug) {
                            debug!(
                                "Initializing authentication, supporting {}",
                                x.methods
                                    .iter()
                                    .map(ToOwned::to_owned)
                                    .collect::<Vec<_>>()
                                    .join(",")
                            );
                        }
                        let msg = AuthenticationResponse::Initialization(
                            handler.on_initialization(x).await?,
                        );
                        self.fire(Request::new(ManagerRequest::Authenticate { id, msg }))
                            .await?;
                    }
                    Authentication::StartMethod(x) => {
                        debug!("Starting authentication method {}", x.method);
                    }
                    Authentication::Challenge(x) => {
                        if log::log_enabled!(Level::Debug) {
                            for question in x.questions.iter() {
                                debug!(
                                    "Received challenge question [{}]: {}",
                                    question.label, question.text
                                );
                            }
                        }
                        let msg = AuthenticationResponse::Challenge(handler.on_challenge(x).await?);
                        self.fire(Request::new(ManagerRequest::Authenticate { id, msg }))
                            .await?;
                    }
                    Authentication::Verification(x) => {
                        debug!("Received verification request {}: {}", x.kind, x.text);
                        let msg =
                            AuthenticationResponse::Verification(handler.on_verification(x).await?);
                        self.fire(Request::new(ManagerRequest::Authenticate { id, msg }))
                            .await?;
                    }
                    Authentication::Info(x) => {
                        info!("{}", x.text);
                    }
                    Authentication::Error(x) => {
                        error!("{}", x.text);
                        if x.is_fatal() {
                            return Err(x.into_io_permission_denied());
                        }
                    }
                    Authentication::Finished => {
                        debug!("Finished authentication for {destination}");
                    }
                },
                ManagerResponse::Launched { destination } => return Ok(destination),
                ManagerResponse::Error { description } => {
                    return Err(io::Error::new(io::ErrorKind::Other, description))
                }
                x => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Got unexpected response: {x:?}"),
                    ))
                }
            }
        }

        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Missing connection confirmation",
        ))
    }

    /// Request that the manager establishes a new connection at the given `destination`
    /// with `options` being passed for destination-specific details.
    ///
    /// The provided `handler` will be used for any authentication requirements when connecting to
    /// the server.
    pub async fn connect(
        &mut self,
        destination: impl Into<Destination>,
        options: impl Into<Map>,
        mut handler: impl AuthHandler + Send,
    ) -> io::Result<ConnectionId> {
        let destination = Box::new(destination.into());
        let options = options.into();
        trace!("connect({}, {})", destination, options);

        let mut mailbox = self
            .mail(ManagerRequest::Connect {
                destination: destination.clone(),
                options,
            })
            .await?;

        // Continue to process authentication challenges and other details until we are either
        // connected or fail
        while let Some(res) = mailbox.next().await {
            match res.payload {
                ManagerResponse::Authenticate { id, msg } => match msg {
                    Authentication::Initialization(x) => {
                        if log::log_enabled!(Level::Debug) {
                            debug!(
                                "Initializing authentication, supporting {}",
                                x.methods
                                    .iter()
                                    .map(ToOwned::to_owned)
                                    .collect::<Vec<_>>()
                                    .join(",")
                            );
                        }
                        let msg = AuthenticationResponse::Initialization(
                            handler.on_initialization(x).await?,
                        );
                        self.fire(Request::new(ManagerRequest::Authenticate { id, msg }))
                            .await?;
                    }
                    Authentication::StartMethod(x) => {
                        debug!("Starting authentication method {}", x.method);
                    }
                    Authentication::Challenge(x) => {
                        if log::log_enabled!(Level::Debug) {
                            for question in x.questions.iter() {
                                debug!(
                                    "Received challenge question [{}]: {}",
                                    question.label, question.text
                                );
                            }
                        }
                        let msg = AuthenticationResponse::Challenge(handler.on_challenge(x).await?);
                        self.fire(Request::new(ManagerRequest::Authenticate { id, msg }))
                            .await?;
                    }
                    Authentication::Verification(x) => {
                        debug!("Received verification request {}: {}", x.kind, x.text);
                        let msg =
                            AuthenticationResponse::Verification(handler.on_verification(x).await?);
                        self.fire(Request::new(ManagerRequest::Authenticate { id, msg }))
                            .await?;
                    }
                    Authentication::Info(x) => {
                        info!("{}", x.text);
                    }
                    Authentication::Error(x) => {
                        error!("{}", x.text);
                        if x.is_fatal() {
                            return Err(x.into_io_permission_denied());
                        }
                    }
                    Authentication::Finished => {
                        debug!("Finished authentication for {destination}");
                    }
                },
                ManagerResponse::Connected { id } => return Ok(id),
                ManagerResponse::Error { description } => {
                    return Err(io::Error::new(io::ErrorKind::Other, description))
                }
                x => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Got unexpected response: {x:?}"),
                    ))
                }
            }
        }

        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Missing connection confirmation",
        ))
    }

    /// Establishes a channel with the server represented by the `connection_id`,
    /// returning a [`RawChannel`] acting as the connection.
    ///
    /// ### Note
    ///
    /// Multiple calls to open a channel against the same connection will result in establishing a
    /// duplicate channel to the same server, so take care when using this method.
    pub async fn open_raw_channel(
        &mut self,
        connection_id: ConnectionId,
    ) -> io::Result<RawChannel> {
        trace!("open_raw_channel({})", connection_id);
        RawChannel::spawn(connection_id, self).await
    }

    /// Retrieves a list of supported capabilities
    pub async fn capabilities(&mut self) -> io::Result<ManagerCapabilities> {
        trace!("capabilities()");
        let res = self.send(ManagerRequest::Capabilities).await?;
        match res.payload {
            ManagerResponse::Capabilities { supported } => Ok(supported),
            ManagerResponse::Error { description } => {
                Err(io::Error::new(io::ErrorKind::Other, description))
            }
            x => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Got unexpected response: {x:?}"),
            )),
        }
    }

    /// Retrieves information about a specific connection
    pub async fn info(&mut self, id: ConnectionId) -> io::Result<ConnectionInfo> {
        trace!("info({})", id);
        let res = self.send(ManagerRequest::Info { id }).await?;
        match res.payload {
            ManagerResponse::Info(info) => Ok(info),
            ManagerResponse::Error { description } => {
                Err(io::Error::new(io::ErrorKind::Other, description))
            }
            x => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Got unexpected response: {x:?}"),
            )),
        }
    }

    /// Kills the specified connection
    pub async fn kill(&mut self, id: ConnectionId) -> io::Result<()> {
        trace!("kill({})", id);
        let res = self.send(ManagerRequest::Kill { id }).await?;
        match res.payload {
            ManagerResponse::Killed => Ok(()),
            ManagerResponse::Error { description } => {
                Err(io::Error::new(io::ErrorKind::Other, description))
            }
            x => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Got unexpected response: {x:?}"),
            )),
        }
    }

    /// Retrieves a list of active connections
    pub async fn list(&mut self) -> io::Result<ConnectionList> {
        trace!("list()");
        let res = self.send(ManagerRequest::List).await?;
        match res.payload {
            ManagerResponse::List(list) => Ok(list),
            ManagerResponse::Error { description } => {
                Err(io::Error::new(io::ErrorKind::Other, description))
            }
            x => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Got unexpected response: {x:?}"),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use distant_auth::DummyAuthHandler;

    use super::*;
    use crate::client::UntypedClient;
    use crate::common::{Connection, InmemoryTransport, Request, Response};

    fn setup() -> (ManagerClient, Connection<InmemoryTransport>) {
        let (client, server) = Connection::pair(100);
        let client = UntypedClient::spawn(client, Default::default()).into_typed_client();
        (client, server)
    }

    #[inline]
    fn test_error() -> io::Error {
        io::Error::new(io::ErrorKind::Interrupted, "test error")
    }

    #[inline]
    fn test_error_response() -> ManagerResponse {
        ManagerResponse::from(test_error())
    }

    #[tokio::test]
    async fn connect_should_report_error_if_receives_error_response() {
        let (mut client, mut transport) = setup();

        tokio::spawn(async move {
            let request = transport
                .read_frame_as::<Request<ManagerRequest>>()
                .await
                .unwrap()
                .unwrap();

            transport
                .write_frame_for(&Response::new(request.id, test_error_response()))
                .await
                .unwrap();
        });

        let err = client
            .connect(
                "scheme://host".parse::<Destination>().unwrap(),
                "key=value".parse::<Map>().unwrap(),
                DummyAuthHandler,
            )
            .await
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert_eq!(err.to_string(), test_error().to_string());
    }

    #[tokio::test]
    async fn connect_should_report_error_if_receives_unexpected_response() {
        let (mut client, mut transport) = setup();

        tokio::spawn(async move {
            let request = transport
                .read_frame_as::<Request<ManagerRequest>>()
                .await
                .unwrap()
                .unwrap();

            transport
                .write_frame_for(&Response::new(request.id, ManagerResponse::Killed))
                .await
                .unwrap();
        });

        let err = client
            .connect(
                "scheme://host".parse::<Destination>().unwrap(),
                "key=value".parse::<Map>().unwrap(),
                DummyAuthHandler,
            )
            .await
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn connect_should_return_id_from_successful_response() {
        let (mut client, mut transport) = setup();

        let expected_id = 999;
        tokio::spawn(async move {
            let request = transport
                .read_frame_as::<Request<ManagerRequest>>()
                .await
                .unwrap()
                .unwrap();

            transport
                .write_frame_for(&Response::new(
                    request.id,
                    ManagerResponse::Connected { id: expected_id },
                ))
                .await
                .unwrap();
        });

        let id = client
            .connect(
                "scheme://host".parse::<Destination>().unwrap(),
                "key=value".parse::<Map>().unwrap(),
                DummyAuthHandler,
            )
            .await
            .unwrap();
        assert_eq!(id, expected_id);
    }

    #[tokio::test]
    async fn info_should_report_error_if_receives_error_response() {
        let (mut client, mut transport) = setup();

        tokio::spawn(async move {
            let request = transport
                .read_frame_as::<Request<ManagerRequest>>()
                .await
                .unwrap()
                .unwrap();

            transport
                .write_frame_for(&Response::new(request.id, test_error_response()))
                .await
                .unwrap();
        });

        let err = client.info(123).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert_eq!(err.to_string(), test_error().to_string());
    }

    #[tokio::test]
    async fn info_should_report_error_if_receives_unexpected_response() {
        let (mut client, mut transport) = setup();

        tokio::spawn(async move {
            let request = transport
                .read_frame_as::<Request<ManagerRequest>>()
                .await
                .unwrap()
                .unwrap();

            transport
                .write_frame_for(&Response::new(request.id, ManagerResponse::Killed))
                .await
                .unwrap();
        });

        let err = client.info(123).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn info_should_return_connection_info_from_successful_response() {
        let (mut client, mut transport) = setup();

        tokio::spawn(async move {
            let request = transport
                .read_frame_as::<Request<ManagerRequest>>()
                .await
                .unwrap()
                .unwrap();

            let info = ConnectionInfo {
                id: 123,
                destination: "scheme://host".parse::<Destination>().unwrap(),
                options: "key=value".parse::<Map>().unwrap(),
            };

            transport
                .write_frame_for(&Response::new(request.id, ManagerResponse::Info(info)))
                .await
                .unwrap();
        });

        let info = client.info(123).await.unwrap();
        assert_eq!(info.id, 123);
        assert_eq!(
            info.destination,
            "scheme://host".parse::<Destination>().unwrap()
        );
        assert_eq!(info.options, "key=value".parse::<Map>().unwrap());
    }

    #[tokio::test]
    async fn list_should_report_error_if_receives_error_response() {
        let (mut client, mut transport) = setup();

        tokio::spawn(async move {
            let request = transport
                .read_frame_as::<Request<ManagerRequest>>()
                .await
                .unwrap()
                .unwrap();

            transport
                .write_frame_for(&Response::new(request.id, test_error_response()))
                .await
                .unwrap();
        });

        let err = client.list().await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert_eq!(err.to_string(), test_error().to_string());
    }

    #[tokio::test]
    async fn list_should_report_error_if_receives_unexpected_response() {
        let (mut client, mut transport) = setup();

        tokio::spawn(async move {
            let request = transport
                .read_frame_as::<Request<ManagerRequest>>()
                .await
                .unwrap()
                .unwrap();

            transport
                .write_frame_for(&Response::new(request.id, ManagerResponse::Killed))
                .await
                .unwrap();
        });

        let err = client.list().await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn list_should_return_connection_list_from_successful_response() {
        let (mut client, mut transport) = setup();

        tokio::spawn(async move {
            let request = transport
                .read_frame_as::<Request<ManagerRequest>>()
                .await
                .unwrap()
                .unwrap();

            let mut list = ConnectionList::new();
            list.insert(123, "scheme://host".parse::<Destination>().unwrap());

            transport
                .write_frame_for(&Response::new(request.id, ManagerResponse::List(list)))
                .await
                .unwrap();
        });

        let list = client.list().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(
            list.get(&123).expect("Connection list missing item"),
            &"scheme://host".parse::<Destination>().unwrap()
        );
    }

    #[tokio::test]
    async fn kill_should_report_error_if_receives_error_response() {
        let (mut client, mut transport) = setup();

        tokio::spawn(async move {
            let request = transport
                .read_frame_as::<Request<ManagerRequest>>()
                .await
                .unwrap()
                .unwrap();

            transport
                .write_frame_for(&Response::new(request.id, test_error_response()))
                .await
                .unwrap();
        });

        let err = client.kill(123).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert_eq!(err.to_string(), test_error().to_string());
    }

    #[tokio::test]
    async fn kill_should_report_error_if_receives_unexpected_response() {
        let (mut client, mut transport) = setup();

        tokio::spawn(async move {
            let request = transport
                .read_frame_as::<Request<ManagerRequest>>()
                .await
                .unwrap()
                .unwrap();

            transport
                .write_frame_for(&Response::new(
                    request.id,
                    ManagerResponse::Connected { id: 0 },
                ))
                .await
                .unwrap();
        });

        let err = client.kill(123).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn kill_should_return_success_from_successful_response() {
        let (mut client, mut transport) = setup();

        tokio::spawn(async move {
            let request = transport
                .read_frame_as::<Request<ManagerRequest>>()
                .await
                .unwrap()
                .unwrap();

            transport
                .write_frame_for(&Response::new(request.id, ManagerResponse::Killed))
                .await
                .unwrap();
        });

        client.kill(123).await.unwrap();
    }
}
