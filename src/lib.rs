use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::time::{Duration, Instant};
use ringbuf::{HeapRb, SharedRb, traits::*};
use std::sync::{Arc, Mutex};

mod utils;
use utils::coletar::ColetarSaida;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const MIN_LEN: f32 = 2.0;

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
        // println!("{:?}: Copiando stream", std::thread::current().id());
        let mut stream = self.stream.as_ref().unwrap();
        // println!("{:?}: Copiado stream", std::thread::current().id());

        let mut seq_le: Vec<u8> = Vec::new();
        seq.iter().for_each(|i| seq_le.extend_from_slice(&i.to_le_bytes()));


        // println!("{:?}: Escrevendo na stream", std::thread::current().id());
        stream.write_all(&seq_le)?;
        stream.flush()?;
        // println!("{:?}: Escrito na stream", std::thread::current().id());

        // println!("{:?}: Lendo da stream", std::thread::current().id());
        // std::thread::sleep(Duration::from_millis(100));
        // stream.peek(buf);
        // let mut resp = [0; 1024];
        // stream.read(&mut resp)?;
        // let vec_resp = Vec::from(&resp);
        // let vec_resp = vec_resp.into_iter().filter(|i| { i != &0_u8 } ).collect::<Vec<u8>>();
        // println!("{:?}: Resposta da stream: {:?}", std::thread::current().id(), resp);     
        // println!("{:?}: Resposta string da stream: {:?}", std::thread::current().id(), String::from_utf8(vec_resp));

        // stream.flush()?; 

        // Ok(String::from_utf8(vec_resp)?)
        Ok(())
    }

    pub fn ler(&mut self) -> Result<String>{
        if self.stream.is_none() { self.iniciar_conexao()? }
        // println!(" === Iniciando stream");
        let mut stream = self.stream.as_ref().unwrap();
        stream.set_read_timeout(Some(Duration::from_millis(100)))?;
        // println!(" === Iniciado stream");

        // println!(" === Tentando read");
        let mut resp = [0; 1024];
        stream.read(&mut resp)?;
        let vec_resp = Vec::from(&resp);
        let vec_resp = vec_resp.into_iter().filter(|i| { i != &0_u8 } ).collect::<Vec<u8>>();
        // println!(" === Read feito");

        stream.flush()?; 

        Ok(String::from_utf8(vec_resp)?)
    }

}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() -> Result<()> {
        let mut sender = Arc::new(Mutex::new(Enviador::new("localhost".to_string(), 43007)));
        let mut coletar_saida = ColetarSaida::new(10.0)?;
        let buffer = coletar_saida.pegar_buffer();
        let samplerate_buffer: usize = coletar_saida.samplerate;
        let samplerate_alvo = 16_000;
        let canais = coletar_saida.canais;
        let razao_samplerates = samplerate_buffer / samplerate_alvo;
        coletar_saida.coletar()?;
                
        // Espera o buffer encher o suficiente pro whisper aceitar
        let mut buffer_cheio: bool = false;
        while !buffer_cheio {
            buffer_cheio = buffer.lock().unwrap().occupied_len() >= (MIN_LEN * samplerate_buffer as f32) as usize;
        }

        let n_sender = Arc::clone(&sender);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(100));
                // println!(" === Tentando leitura");
                let mut sender = n_sender.lock().unwrap();
                let Ok(res) = sender.ler() else { 
                    // println!(" === Nada pra ler");
                    continue 
                };
                println!(" === Resposta: {}", res);
            }
        });
        
        loop {
            // Espera o buffer encher antes de fazer qualque coisa
            println!("Nova iteração");
            let inicio_coleta_buffer = Instant::now();
            let mut buffer_cheio: bool = false;
            while !buffer_cheio {
                let buffer_lock = buffer.lock().unwrap();
                buffer_cheio = buffer_lock.occupied_len() >= (MIN_LEN * samplerate_buffer as f32) as usize;
                
                if buffer_lock.occupied_len() == 0 { continue; }

                // Se o áudio estiver parado, envia oq tiver aí.
                if inicio_coleta_buffer.elapsed() > Duration::from_millis((MIN_LEN * 1000.0) as u64) {
                    println!("Mt tempo sem nova amostra, mandando oq tem aqui");
                    break;
                }
            }

            let mut copia_buffer: Vec<i16> = Vec::new();            
            {
                // Escopo que copia todo o buffer para uma lista de copia
                // Escopo pra poder travar o mutex e impedir que seja adicionado mais alguma coisa lá
                let mut buffer = buffer.lock().unwrap();
                println!("Buffer tem {} segundos de áudio)", buffer.occupied_len() as f32 / samplerate_buffer as f32);

                // println!("Copiando buffer");
                buffer.iter().for_each(|i| copia_buffer.push(i.clone()) );
                println!("{} amostras copiadas", copia_buffer.iter().count());

                // Tira todas as amostras até que sobre apenas o necessário para sobreposição
                // println!("Existem {} ({}s) amostras no buffer", buffer.occupied_len(), buffer.occupied_len() / samplerate_buffer);
                while buffer.occupied_len() > 0 {
                    buffer.try_pop();
                }
            }

            // Remoção dos canais extras
            match canais {
                1 => {  }
                2 => {
                    copia_buffer = copia_buffer.iter().enumerate()
                        .filter(|i| i.0 % 2 == 0).map(|(_, &a)| a).collect();
                }
                _ => {
                    copia_buffer = copia_buffer.iter().enumerate()
                        .filter(|i| i.0 % canais as usize == 0).map(|(_, &a)| a).collect();
                }
            }

            // Resampling seboso
            let copia_buffer: Vec<i16> = copia_buffer.iter().enumerate()
                .filter(|i| i.0 % razao_samplerates == 0).map(|(_, &a)| a).collect();

            println!("Pré envio");
            let _ = match sender.lock().unwrap().enviar_sequencia(copia_buffer) {
                Ok(a) => {a},
                Err(e) => { println!("Erro ao coisar: {}", e); continue; }
            };
            println!("Pós envio");
            // println!("Resposta da sequência: {:?}", resp);
        }
        Ok(())
    }
}
