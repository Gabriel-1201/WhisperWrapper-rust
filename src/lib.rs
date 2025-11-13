use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::time::{Duration, Instant};
use ringbuf::{HeapRb, SharedRb, traits::*};
use std::sync::{Arc, Mutex};
use regex::Regex;

mod utils;
use utils::coletor::ColetorSaida;
use utils::enviador::Enviador;
use utils::buffer_texto::BufferTexto;


type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const MIN_LEN: f32 = 0.3;
// const LIMITE_CARACTERES: u8 = 255;

struct WhisperWrapper {
    coletor: ColetorSaida,
    enviador: Arc<Mutex<Enviador>>,
    rodando: Arc<Mutex<bool>>,
    buffer_texto: Arc<Mutex<BufferTexto>>
}

impl WhisperWrapper {
    pub fn new(host: String, porta: u32, limite_caracteres: u32) -> Result<Self> {
        Ok(
            Self { 
                coletor: ColetorSaida::new(30.0)?, 
                enviador: Arc::new(Mutex::new(Enviador::new(host, porta))),
                rodando: Arc::new(Mutex::new(false)),
                buffer_texto: Arc::new(Mutex::new(BufferTexto::new(limite_caracteres)))
            }
        )
    }

    pub fn iniciar_envio(&mut self) -> Result<()> {
        if self.rodando.lock().unwrap().eq(&false) {
            self.coletor.coletar()?;
        }

        let samplerate_buffer: usize = self.coletor.samplerate;
        let samplerate_alvo = 16_000;
        let canais = self.coletor.canais;
        let razao_samplerates = samplerate_buffer / samplerate_alvo;
        let buffer_audio = self.coletor.pegar_buffer();
        
        let tx_enviador = Arc::clone(&self.enviador);
        let rx_enviador = Arc::clone(&self.enviador);

        let rx_buffer = Arc::clone(&self.buffer_texto);

        std::thread::spawn(move || loop {                    
            // Espera o buffer encher o suficiente pro whisper aceitar
            let mut buffer_cheio: bool = false;
            while !buffer_cheio {
                buffer_cheio = buffer_audio.lock().unwrap().occupied_len() >= (MIN_LEN * samplerate_buffer as f32) as usize;
            }
            
            loop {
                // Espera o buffer encher antes de fazer qualque coisa
                // println!(" -- Nova iteração -- ");
                let inicio_coleta_buffer = Instant::now();
                let mut buffer_cheio: bool = false;
                while !buffer_cheio {
                    let buffer_lock = buffer_audio.lock().unwrap();
                    buffer_cheio = buffer_lock.occupied_len() >= (MIN_LEN * samplerate_buffer as f32) as usize;
                    
                    if buffer_lock.occupied_len() == 0 { continue; }

                    // Se o áudio estiver parado, envia oq tiver aí.
                    if inicio_coleta_buffer.elapsed() > Duration::from_millis((MIN_LEN * 1000.0) as u64) {
                        // println!("Mt tempo sem nova amostra, mandando oq tem aqui");
                        break;
                    }
                }

                let mut copia_buffer: Vec<i16> = Vec::new();            
                {
                    // Escopo que copia todo o buffer para uma lista de copia
                    // Escopo pra poder travar o mutex e impedir que seja adicionado mais alguma coisa lá
                    let mut buffer = buffer_audio.lock().unwrap();
                    // println!("Buffer tem {} segundos de áudio)", buffer.occupied_len() as f32 / samplerate_buffer as f32);

                    // println!("Copiando buffer");
                    buffer.iter().for_each(|i| copia_buffer.push(i.clone()) );
                    // println!("{} amostras copiadas", copia_buffer.iter().count());

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

                // println!("Preparando envio");
                match tx_enviador.lock().unwrap().enviar_sequencia(copia_buffer) {
                    Ok(_) => {
                    }
                    Err(e) => { 
                            println!(" === Erro ao enviar: {}", e); 
                            continue; 
                    }
                };
            }
        });

        std::thread::spawn(move || {
            loop {
                // Esse sleep é extremamente necessário, senão ele fica fominha com o enviador 
                std::thread::sleep(Duration::from_millis(100));

                // println!(" === Tentando leitura");
                let mut sender = rx_enviador.lock().unwrap();
                let Ok(res) = sender.ler() else { 
                    // println!(" === Nada pra ler");
                    continue 
                };
                let re = Regex::new(r"^\d*\s\d*\s(.*)\n").unwrap();
                let Some(a) = re.captures(res.as_str()) else {
                    continue
                };
                let Some(texto) = a.get(1) else {
                    continue
                };

                rx_buffer.lock().unwrap().push(texto.as_str());
                // println!("{:?}", texto.as_str());
            }
        });

        let mut rodando = self.rodando.lock().unwrap();
        *rodando = true;
        Ok(())
    }

    pub fn pegar_transcricao(&mut self) -> String {
        self.buffer_texto.lock().unwrap().get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() -> Result<()> {
        let mut a = WhisperWrapper::new("localhost".to_string(), 43007, 512)?;
        a.iniciar_envio()?;

        loop {
            let i = a.pegar_transcricao();
            if !i.is_empty() {
                println!("Trancrição: {}", i);
            }
            std::thread::sleep(Duration::from_millis(500));
        }

        // a.pegar_transcricao();
        // let mut sender = Arc::new(Mutex::new(Enviador::new("localhost".to_string(), 43007)));
        // // let mut sender = Enviador::new("localhost".to_string(), 43007);
        // let mut coletar_saida = ColetorSaida::new(10.0)?;
        // let buffer = coletar_saida.pegar_buffer();
        // let samplerate_buffer: usize = coletar_saida.samplerate;
        // let samplerate_alvo = 16_000;
        // let canais = coletar_saida.canais;
        // let razao_samplerates = samplerate_buffer / samplerate_alvo;
        // coletar_saida.coletar()?;
                
        // // Espera o buffer encher o suficiente pro whisper aceitar
        // let mut buffer_cheio: bool = false;
        // while !buffer_cheio {
        //     buffer_cheio = buffer.lock().unwrap().occupied_len() >= (MIN_LEN * samplerate_buffer as f32) as usize;
        // }

        // let n_sender = Arc::clone(&sender);
        // std::thread::spawn(move || {
        //     loop {
        //         std::thread::sleep(Duration::from_millis(100));
        //         // println!(" === Tentando leitura");
        //         let mut sender = n_sender.lock().unwrap();
        //         let Ok(res) = sender.ler() else { 
        //             // println!(" === Nada pra ler");
        //             continue 
        //         };
        //         let re = Regex::new(r"^\d*\s\d*\s(.*)\n").unwrap();
        //         let Some(a) = re.captures(res.as_str()) else {
        //             continue
        //         };
        //         let Some(texto) = a.get(1) else {
        //             continue
        //         };
        //         println!("{:?}", texto.as_str());
        //     }
        // });
        
        // loop {
        //     // Espera o buffer encher antes de fazer qualque coisa
        //     println!(" -- Nova iteração -- ");
        //     let inicio_coleta_buffer = Instant::now();
        //     let mut buffer_cheio: bool = false;
        //     while !buffer_cheio {
        //         let buffer_lock = buffer.lock().unwrap();
        //         buffer_cheio = buffer_lock.occupied_len() >= (MIN_LEN * samplerate_buffer as f32) as usize;
                
        //         if buffer_lock.occupied_len() == 0 { continue; }

        //         // Se o áudio estiver parado, envia oq tiver aí.
        //         if inicio_coleta_buffer.elapsed() > Duration::from_millis((MIN_LEN * 1000.0) as u64) {
        //             // println!("Mt tempo sem nova amostra, mandando oq tem aqui");
        //             break;
        //         }
        //     }

        //     let mut copia_buffer: Vec<i16> = Vec::new();            
        //     {
        //         // Escopo que copia todo o buffer para uma lista de copia
        //         // Escopo pra poder travar o mutex e impedir que seja adicionado mais alguma coisa lá
        //         let mut buffer = buffer.lock().unwrap();
        //         println!("Buffer tem {} segundos de áudio)", buffer.occupied_len() as f32 / samplerate_buffer as f32);

        //         // println!("Copiando buffer");
        //         buffer.iter().for_each(|i| copia_buffer.push(i.clone()) );
        //         println!("{} amostras copiadas", copia_buffer.iter().count());

        //         // Tira todas as amostras até que sobre apenas o necessário para sobreposição
        //         // println!("Existem {} ({}s) amostras no buffer", buffer.occupied_len(), buffer.occupied_len() / samplerate_buffer);
        //         while buffer.occupied_len() > 0 {
        //             buffer.try_pop();
        //         }
        //     }

        //     // Remoção dos canais extras
        //     match canais {
        //         1 => {  }
        //         2 => {
        //             copia_buffer = copia_buffer.iter().enumerate()
        //                 .filter(|i| i.0 % 2 == 0).map(|(_, &a)| a).collect();
        //         }
        //         _ => {
        //             copia_buffer = copia_buffer.iter().enumerate()
        //                 .filter(|i| i.0 % canais as usize == 0).map(|(_, &a)| a).collect();
        //         }
        //     }

        //     // Resampling seboso
        //     let copia_buffer: Vec<i16> = copia_buffer.iter().enumerate()
        //         .filter(|i| i.0 % razao_samplerates == 0).map(|(_, &a)| a).collect();

        //     // println!("Preparando envio");
        //     match sender.lock().unwrap().enviar_sequencia(copia_buffer) {
        //         Ok(_) => {
        //             // println!("Enviado");
        //             // match sender.ler() {
        //             //     Ok(texto) => { 
        //             //         println!(" === {}", texto);
        //             //     },
        //             //     Err(e) => { 
        //             //         println!(" === Erro ao ler: {}", e);
        //             //     }
        //             // }
        //         }
        //         Err(e) => { 
        //                 println!(" === Erro ao enviar: {}", e); 
        //                 continue; 
        //         }
        //     };
        // }
        Ok(())
    }
}
