use cpal::{
    Stream, 
    traits::{
        DeviceTrait, 
        HostTrait, 
        StreamTrait
    }
};
use ringbuf::{HeapRb, SharedRb, traits::*};
use std::sync::{
    Arc, 
    Mutex
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub struct ColetorSaida {
    buffer: Arc<Mutex<HeapRb<i16>>>,
    pub canais: u16,
    pub samplerate: usize,
    pub rodando: bool,
    stream: Option<Stream>
}

impl ColetorSaida {
    pub fn new(tam_buffer_segundos: f32) -> Result<Self> {
        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            return Err("Sem dispositivo de entrada!".into())
        };

        let mut supported_configs_range = device.supported_output_configs()?;
        println!("ColetarSaida: Tentando pegar canal de 48KHz");
        let supported_config = supported_configs_range.next()
            .ok_or("Dispositivo de entrada não possui nenhuma configuração suportada")?
            .with_max_sample_rate();
            // .try_with_sample_rate(SampleRate(48000)).ok_or("Não tem 48KHz aqui dnv")?;

        let samplerate = supported_config.sample_rate().0 as usize;
        let canais = supported_config.channels();
        if canais == 0 { return Err("Nenhum canal de áudio aqui".into()) }
        let tam_total_buffer = (samplerate as f32 * tam_buffer_segundos * canais as f32).floor() as usize;

        Ok(ColetorSaida { 
            buffer: Arc::new(Mutex::new(HeapRb::<i16>::new(tam_total_buffer))), 
            samplerate,
            canais: canais,
            rodando: false, 
            stream: None 
        })
    }

    pub fn pegar_buffer(&mut self) -> Arc<Mutex<HeapRb<i16>>>{
        return Arc::clone(&self.buffer)
    }

    pub fn coletar(&mut self) -> Result<()> {
        if !self.rodando {
            let host = cpal::default_host();
            let Some(device) = host.default_output_device() else {
                return Err("Sem dispositivo de saída!".into())
            };
            println!("ColetarSaida: Dispositivo usado: {:?}", device.name());

            let mut supported_configs_range = device.supported_output_configs()?;
            println!("ColetarSaida: Tentando pegar canal de 48KHz");
            let supported_config = supported_configs_range.next()
            .ok_or("Dispositivo de entrada não possui nenhuma configuração suportada")?
            .with_max_sample_rate();

            let config = supported_config.into(); 
            println!("Configuração usada: {:?}", config);

            let buffer = Arc::clone(&self.buffer);
            let stream = device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        for &sample in data {
                            buffer.lock().unwrap().push_overwrite(sample);
                        }
                    },
                    move |err| { println!("ColetarSaida: Erro de callback: {}", err) },
                    None
            )?;
            stream.play()?;
            self.stream = Some(stream);
            self.rodando = true;
        }
        Ok(())
    }

    pub fn parar(&mut self) -> Result<()> {
        match &self.stream {
            Some(stream) => { stream.pause()? },
            None => {}
        }
        Ok(())
    }

    pub fn retomar(&mut self) -> Result<()> {
        match &self.stream {
            Some(stream) => { stream.play()? },
            None => {}
        }
        Ok(())
    }
}
