use std::io::{Read, Write};

use crate::config::WebexConfig;
use libssh_rs::{
    AuthMethods, AuthStatus, Error, KnownHosts, Metadata, OpenFlags, PublicKeyHashType, Session,
    SshKey, SshOption, SshResult, get_input,
};

pub fn list_files(config: &WebexConfig, path: &str) -> Result<Vec<Metadata>, Error> {
    let sess = start_session(&config)?;
    let sftp = sess.sftp()?;
    let files = sftp.read_dir(&format!(
        "{}/{}",
        config.peer.root.as_deref().unwrap_or("/"),
        path
    ))?;

    Ok(files)
}

pub enum CopyFileEvent {
    Start(usize),
    Written(usize),
}

pub fn copy_file(
    config: &WebexConfig,
    src: &str,
    dst: &str,
    callback: impl Fn(CopyFileEvent),
) -> Result<(), Error> {
    let sess = start_session(&config)?;
    let sftp = sess.sftp()?;
    let filename = format!("{}/{}", config.peer.root.as_deref().unwrap_or("/"), src);

    let mut file = sftp.open(&filename, OpenFlags::READ_ONLY, 0644)?;
    let metadata = file.metadata()?;
    if let Some(size) = metadata.len() {
        callback(CopyFileEvent::Start(size as usize));
    }

    let locfile_path = format!("{}/{}", config.local.path.as_deref().unwrap_or("."), dst);
    let mut locfile = std::fs::File::create(&locfile_path).unwrap();
    loop {
        let mut buf = [0; 32 * 1024];
        let read = file.read(&mut buf)?;
        callback(CopyFileEvent::Written(read));
        locfile.write(&buf).unwrap();
        if read == 0 {
            break;
        }
    }

    Ok(())
}

fn start_session(config: &WebexConfig) -> SshResult<Session> {
    let sess = Session::new().unwrap();
    sess.set_auth_callback(|prompt, echo, verify, identity| {
        let prompt = match identity {
            Some(ident) => format!("{} ({}): ", prompt, ident),
            None => prompt.to_string(),
        };
        get_input(&prompt, None, echo, verify)
            .ok_or_else(|| Error::Fatal("reading password".to_string()))
    });

    sess.set_option(SshOption::Hostname(config.peer.hostname.clone()))?;
    sess.options_parse_config(None)?;
    sess.connect()?;
    verify_known_hosts(&sess)?;

    let ssh_key = if let Some(path) = config.peer.key_path.as_deref() {
        Some(SshKey::from_privkey_file(path, None)?)
    } else {
        None
    };
    authenticate(&sess, config.peer.user.as_deref(), ssh_key)?;
    Ok(sess)
}

fn verify_known_hosts(sess: &Session) -> SshResult<()> {
    let key = sess
        .get_server_public_key()?
        .get_public_key_hash_hexa(PublicKeyHashType::Sha256)?;

    match sess.is_known_server()? {
        KnownHosts::Ok => Ok(()),
        KnownHosts::NotFound | KnownHosts::Unknown => {
            eprintln!("The server is not a known host. Do you trust the host key?");
            eprintln!("Public key hash: {}", key);

            let input = prompt_stdin("Enter yes to trust the key: ")?;
            if input == "yes" {
                sess.update_known_hosts_file()
            } else {
                Err(Error::Fatal("untrusted server".to_string()))
            }
        }
        KnownHosts::Changed => {
            eprintln!("The key for the server has changed. It is now:");
            eprintln!("{}", key);
            Err(Error::Fatal("host key changed".to_string()))
        }
        KnownHosts::Other => {
            eprintln!("The host key for this server was not found, but another");
            eprintln!("type of key exists. An attacker might change the default");
            eprintln!("server key to confuse your client into thinking the key");
            eprintln!("does not exist");
            Err(Error::Fatal("host key has wrong type".to_string()))
        }
    }
}

fn prompt(prompt: &str, echo: bool) -> SshResult<String> {
    get_input(prompt, None, echo, false).ok_or_else(|| Error::Fatal("reading password".to_string()))
}

fn prompt_stdin(prompt: &str) -> SshResult<String> {
    eprintln!("{}", prompt);
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn authenticate(sess: &Session, user_name: Option<&str>, key: Option<SshKey>) -> SshResult<()> {
    match sess.userauth_none(user_name)? {
        AuthStatus::Success => return Ok(()),
        _ => {}
    }

    loop {
        let auth_methods = sess.userauth_list(user_name)?;

        if auth_methods.contains(AuthMethods::PUBLIC_KEY) {
            if let Some(key) = key {
                match sess.userauth_publickey(user_name, &key)? {
                    AuthStatus::Success => return Ok(()),
                    _ => {}
                }
            } else {
                match sess.userauth_public_key_auto(None, None)? {
                    AuthStatus::Success => return Ok(()),
                    _ => {}
                }
            }
        }

        if auth_methods.contains(AuthMethods::INTERACTIVE) {
            loop {
                match sess.userauth_keyboard_interactive(None, None)? {
                    AuthStatus::Success => return Ok(()),
                    AuthStatus::Info => {
                        let info = sess.userauth_keyboard_interactive_info()?;
                        if !info.instruction.is_empty() {
                            eprintln!("{}", info.instruction);
                        }
                        let mut answers = vec![];
                        for p in &info.prompts {
                            answers.push(prompt(&p.prompt, p.echo)?);
                        }
                        sess.userauth_keyboard_interactive_set_answers(&answers)?;

                        continue;
                    }
                    AuthStatus::Denied => {
                        break;
                    }
                    status => {
                        return Err(Error::Fatal(format!(
                            "interactive auth status: {:?}",
                            status
                        )));
                    }
                }
            }
        }

        if auth_methods.contains(AuthMethods::PASSWORD) {
            let pw = prompt("Password: ", false)?;

            match sess.userauth_password(user_name, Some(&pw))? {
                AuthStatus::Success => return Ok(()),
                status => return Err(Error::Fatal(format!("password auth status: {:?}", status))),
            }
        }

        return Err(Error::Fatal("unhandled auth case".to_string()));
    }
}
