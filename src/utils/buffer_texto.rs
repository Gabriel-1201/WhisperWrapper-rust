pub struct BufferTexto {
    limite: u32,
    texto: String
}

impl BufferTexto {
    pub fn new(limite: u32) -> Self {
        Self { texto: String::new(), limite }
    }

    pub fn push(&mut self, string: &str) {
        self.texto.push_str(string);
        self.truncate();
    }

    pub fn get(&self) -> String {
        self.texto.clone()
    }

    fn truncate(&mut self) {
        if self.texto.len() >= self.limite as usize {
            let mut i = self.texto.chars().rev().collect::<String>();
            i.truncate(self.limite as usize);
            self.texto = i.chars().rev().collect::<String>();
        }
    }
}