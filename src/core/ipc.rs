use std::path::Path;

use nix::{
    sys::socket::{bind, connect, listen, socket, AddressFamily, SockAddr, SockFlag, SockType},
    unistd::{close, read, write},
};

use crate::core::common::{Error, ErrorType, Result};

pub struct IpcParent {
    fd: i32,
    sock_path: String,
}

impl IpcParent {
    pub fn new(path: &String) -> Result<IpcParent> {
        let socket_raw_fd = socket(
            AddressFamily::Unix,
            SockType::SeqPacket,
            SockFlag::SOCK_CLOEXEC,
            None,
        )
        .map_err(|_| Error {
            msg: "unable to create IPC socket".to_string(),
            err_type: ErrorType::Runtime,
        })?;

        let sockaddr = SockAddr::new_unix(Path::new(path)).map_err(|_| Error {
            msg: "unable to create unix socket".to_string(),
            err_type: ErrorType::Runtime,
        })?;

        bind(socket_raw_fd, &sockaddr).map_err(|_| Error {
            msg: "unable to bind IPC socket".to_string(),
            err_type: ErrorType::Runtime,
        })?;

        listen(socket_raw_fd, 10).map_err(|err| Error {
            msg: format!("unable to listen IPC socket {}", err),
            err_type: ErrorType::Runtime,
        })?;
        Ok(IpcParent {
            fd: socket_raw_fd,
            sock_path: path.clone(),
        })
    }

    pub fn wait(&self) -> Result<String> {
        let child_socket = nix::sys::socket::accept(self.fd).map_err(|_| Error {
            msg: "unable to accept incoming socket".to_string(),
            err_type: ErrorType::Runtime,
        })?;

        let mut buf = [0; 1024];
        let num = read(child_socket, &mut buf).unwrap();

        match std::str::from_utf8(&buf[0..num]) {
            Ok(str) => Ok(str.trim().to_string()),
            Err(_) => Err(Error {
                msg: "error while converting byte to string {}".to_string(),
                err_type: ErrorType::Runtime,
            }),
        }
    }

    pub fn close(&self) -> Result<()> {
        close(self.fd).map_err(|_| Error {
            msg: "error closing socket".to_string(),
            err_type: ErrorType::Runtime,
        })?;

        std::fs::remove_file(&self.sock_path).map_err(|_| Error {
            msg: "error removing socket".to_string(),
            err_type: ErrorType::Runtime,
        })?;

        Ok(())
    }
}

pub struct IpcChild {
    fd: i32,
}

impl IpcChild {
    pub fn new(path: &String) -> Result<IpcChild> {
        let socket_raw_fd = socket(
            AddressFamily::Unix,
            SockType::SeqPacket,
            SockFlag::SOCK_CLOEXEC,
            None,
        )
        .map_err(|_| Error {
            msg: "unable to create IPC socket".to_string(),
            err_type: ErrorType::Runtime,
        })?;

        let sockaddr = SockAddr::new_unix(Path::new(path)).map_err(|_| Error {
            msg: "unable to create unix socket".to_string(),
            err_type: ErrorType::Runtime,
        })?;

        connect(socket_raw_fd, &sockaddr).map_err(|_| Error {
            msg: "unable to connect to unix socket".to_string(),
            err_type: ErrorType::Runtime,
        })?;

        Ok(IpcChild { fd: socket_raw_fd })
    }

    pub fn notify(&self, msg: &String) -> Result<()> {
        write(self.fd, msg.as_bytes()).map_err(|_| Error {
            msg: "unable to write to unix socket".to_string(),
            err_type: ErrorType::Runtime,
        })?;
        Ok(())
    }

    pub fn close(&self) -> Result<()> {
        close(self.fd).map_err(|_| Error {
            msg: "error closing socket".to_string(),
            err_type: ErrorType::Runtime,
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use crate::core::ipc::IpcChild;

    use super::IpcParent;

    #[test]
    #[serial]
    fn notify() {
        let path = String::from("./tmp.sock");

        let parent = IpcParent::new(&path).unwrap();

        let _ = std::thread::spawn(move || {
            let child = IpcChild::new(&path).unwrap();
            child.notify(&String::from("hello")).unwrap();
            child.close().unwrap();
        });

        let msg = parent.wait().unwrap();
        parent.close().unwrap();
        assert_eq!(String::from("hello"), msg);
    }

    #[test]
    #[serial]
    fn duplicate_sock() {
        let path = String::from("./tmp.sock");

        let parent = IpcParent::new(&path).unwrap();
        match IpcParent::new(&path) {
            Ok(_) => panic!(),
            Err(_) => assert!(true),
        }

        parent.close().unwrap();
    }

    #[test]
    #[serial]
    fn two_socks() {
        let path1 = String::from("./tmp1.sock");
        let path2 = String::from("./tmp2.sock");

        let parent1 = IpcParent::new(&path1).unwrap();
        let parent2 = IpcParent::new(&path2).unwrap();

        let child1 = IpcChild::new(&path1).unwrap();
        let child2 = IpcChild::new(&path2).unwrap();

        child1.close().unwrap();
        child2.close().unwrap();

        parent1.close().unwrap();
        parent2.close().unwrap();
    }
}
