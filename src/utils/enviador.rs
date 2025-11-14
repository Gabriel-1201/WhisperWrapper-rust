use std::net::TcpStream;
use std::io::{Read, Write};
use std::time::Duration;
// use ringbuf::{HeapRb, SharedRb, traits::*};
// use std::sync::{Arc, Mutex};
// use regex::Regex;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub struct Enviador {
    host: String,
    porta: u32,
    stream: Option<TcpStream>
}

impl Enviador {
    pub fn new(host: String, porta: u32) -> Self {
        Enviador { host, porta, stream: None }
    }

    fn iniciar_conexao(&mut self) -> Result<()> {
        let i = TcpStream::connect(format!("{}:{}", self.host, self.porta))?;
        i.set_write_timeout(Some(Duration::from_secs(15)))?;
        i.set_read_timeout(Some(Duration::from_secs(15)))?;

        self.stream = Some(i);
        Ok(())
    }

    pub fn enviar_sequencia(&mut self, seq: Vec<i16>) -> Result<()>{
        if self.stream.is_none() { self.iniciar_conexao()? }
        let mut stream = self.stream.as_ref().unwrap();

        let mut seq_le: Vec<u8> = Vec::new();
        seq.iter().for_each(|i| seq_le.extend_from_slice(&i.to_le_bytes()));

        stream.write_all(&seq_le)?;
        stream.flush()?;
        Ok(())
    }

    pub fn ler(&mut self) -> Result<String>{
        if self.stream.is_none() { self.iniciar_conexao()? }
        let mut stream = self.stream.as_ref().unwrap();
        stream.set_read_timeout(Some(Duration::from_millis(100)))?;
        let mut resp = [0; 1024];
        stream.read(&mut resp)?;
        let vec_resp = Vec::from(&resp);
        let vec_resp = vec_resp.into_iter().filter(|i| { i != &0_u8 } ).collect::<Vec<u8>>();

        stream.flush()?; 

        Ok(String::from_utf8(vec_resp)?)
    }

}