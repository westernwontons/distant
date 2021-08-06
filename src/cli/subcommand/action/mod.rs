use crate::{
    cli::opt::{ActionSubcommand, CommonOpt, Mode, SessionInput},
    core::{
        data::{Request, ResponsePayload},
        net::{Client, DataStream, TransportError},
        session::{Session, SessionFile},
        utils,
    },
};
use derive_more::{Display, Error, From};
use log::*;
use tokio::{io, time::Duration};

pub(crate) mod inner;

#[derive(Debug, Display, Error, From)]
pub enum Error {
    IoError(io::Error),
    TransportError(TransportError),

    #[display(fmt = "Non-interactive but no operation supplied")]
    MissingOperation,
}

pub fn run(cmd: ActionSubcommand, opt: CommonOpt) -> Result<(), Error> {
    let rt = tokio::runtime::Runtime::new()?;

    rt.block_on(async { run_async(cmd, opt).await })
}

async fn run_async(cmd: ActionSubcommand, opt: CommonOpt) -> Result<(), Error> {
    let timeout = opt.to_timeout_duration();

    match cmd.session {
        SessionInput::Environment => {
            start(
                cmd,
                Client::tcp_connect_timeout(Session::from_environment()?, timeout).await?,
                timeout,
            )
            .await
        }
        SessionInput::File => {
            let path = cmd.session_data.session_file.clone();
            start(
                cmd,
                Client::tcp_connect_timeout(SessionFile::load_from(path).await?.into(), timeout)
                    .await?,
                timeout,
            )
            .await
        }
        SessionInput::Pipe => {
            start(
                cmd,
                Client::tcp_connect_timeout(Session::from_stdin()?, timeout).await?,
                timeout,
            )
            .await
        }
        #[cfg(unix)]
        SessionInput::Socket => {
            let path = cmd.session_data.session_socket.clone();
            start(
                cmd,
                Client::unix_connect_timeout(path, None, timeout).await?,
                timeout,
            )
            .await
        }
        #[cfg(not(unix))]
        SessionInput::Socket => unreachable!(),
    }
}

async fn start<T>(
    cmd: ActionSubcommand,
    mut client: Client<T>,
    timeout: Duration,
) -> Result<(), Error>
where
    T: DataStream + 'static,
{
    if !cmd.interactive && cmd.operation.is_none() {
        return Err(Error::MissingOperation);
    }

    // Make up a tenant name
    let tenant = utils::new_tenant();

    // Special conditions for continuing to process responses
    let mut is_proc_req = false;
    let mut proc_id = 0;

    if let Some(req) = cmd
        .operation
        .map(|payload| Request::new(tenant.as_str(), payload))
    {
        is_proc_req = req.payload.is_proc_run();

        debug!("Client sending request: {:?}", req);
        let res = client.send_timeout(req, timeout).await?;

        // Store the spawned process id for using in sending stdin (if we spawned a proc)
        proc_id = match &res.payload {
            ResponsePayload::ProcStart { id } => *id,
            _ => 0,
        };

        inner::format_response(cmd.mode, res)?.print();
    }

    // If we are executing a process, we want to continue interacting via stdin and receiving
    // results via stdout/stderr
    //
    // If we are interactive, we want to continue looping regardless
    if is_proc_req || cmd.interactive {
        let config = match cmd.mode {
            Mode::Json => inner::LoopConfig::Json,
            Mode::Shell if cmd.interactive => inner::LoopConfig::Shell,
            Mode::Shell => inner::LoopConfig::Proc { id: proc_id },
        };
        inner::interactive_loop(client, tenant, config).await?;
    }

    Ok(())
}
