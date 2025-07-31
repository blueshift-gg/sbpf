use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Default, Clone)]
pub struct CompilationMetrics {
    pub lex_time: Duration,
    pub parse_time: Duration,
    pub validate_time: Duration,
    pub codegen_time: Duration,
    pub total_time: Duration,
    pub instruction_count: usize,
    pub bytecode_size: usize,
    pub memory_usage: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub optimization_passes: Vec<String>,
    pub performance_notes: Vec<String>,
}

impl CompilationMetrics {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn print_report(&self) {
        println!("🚀 Compilation Report");
        println!("═══════════════════════════════════════");
        println!("⏱️  Total time: {:?}", self.total_time);
        println!("📊 Instructions: {}", self.instruction_count);
        println!("💾 Bytecode size: {} bytes", self.bytecode_size);
        println!("🧠 Memory usage: {} bytes", self.memory_usage);
        println!("");
        println!("📈 Performance Breakdown:");
        println!("  🔍 Lex time: {:?} ({:.1}%)", 
                self.lex_time, 
                self.percentage(self.lex_time));
        println!("  🧩 Parse time: {:?} ({:.1}%)", 
                self.parse_time, 
                self.percentage(self.parse_time));
        println!("  ✅ Validate time: {:?} ({:.1}%)", 
                self.validate_time, 
                self.percentage(self.validate_time));
        println!("  ⚡ Codegen time: {:?} ({:.1}%)", 
                self.codegen_time, 
                self.percentage(self.codegen_time));
        println!("");
        
        if !self.optimization_passes.is_empty() {
            println!("🔧 Optimizations applied:");
            for pass in &self.optimization_passes {
                println!("  • {}", pass);
            }
            println!("");
        }
        
        if self.error_count > 0 {
            println!("❌ Errors: {}", self.error_count);
        }
        
        if self.warning_count > 0 {
            println!("⚠️  Warnings: {}", self.warning_count);
        }
        
        if !self.performance_notes.is_empty() {
            println!("");
            println!("💡 Performance Notes:");
            for note in &self.performance_notes {
                println!("  • {}", note);
            }
        }
        
        println!("═══════════════════════════════════════");
    }
    
    fn percentage(&self, duration: Duration) -> f64 {
        if self.total_time.as_nanos() == 0 {
            0.0
        } else {
            (duration.as_nanos() as f64 / self.total_time.as_nanos() as f64) * 100.0
        }
    }
    
    pub fn add_performance_note(&mut self, note: &str) {
        self.performance_notes.push(note.to_string());
    }
    
    pub fn add_optimization_pass(&mut self, pass: &str) {
        self.optimization_passes.push(pass.to_string());
    }
}

#[derive(Debug)]
pub struct PerformanceProfiler {
    start_time: Instant,
    checkpoints: HashMap<String, Instant>,
    metrics: CompilationMetrics,
}

impl PerformanceProfiler {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            checkpoints: HashMap::new(),
            metrics: CompilationMetrics::new(),
        }
    }
    
    pub fn checkpoint(&mut self, name: &str) {
        self.checkpoints.insert(name.to_string(), Instant::now());
    }
    
    pub fn end_checkpoint(&mut self, name: &str) {
        if let Some(start) = self.checkpoints.remove(name) {
            let duration = start.elapsed();
            match name {
                "lex" => self.metrics.lex_time = duration,
                "parse" => self.metrics.parse_time = duration,
                "validate" => self.metrics.validate_time = duration,
                "codegen" => self.metrics.codegen_time = duration,
                _ => {}
            }
        }
    }
    
    pub fn finish(&mut self) -> CompilationMetrics {
        self.metrics.total_time = self.start_time.elapsed();
        self.metrics.clone()
    }
    
    pub fn set_instruction_count(&mut self, count: usize) {
        self.metrics.instruction_count = count;
    }
    
    pub fn set_bytecode_size(&mut self, size: usize) {
        self.metrics.bytecode_size = size;
    }
    
    pub fn set_memory_usage(&mut self, usage: usize) {
        self.metrics.memory_usage = usage;
    }
    
    pub fn increment_error_count(&mut self) {
        self.metrics.error_count += 1;
    }
    
    pub fn increment_warning_count(&mut self) {
        self.metrics.warning_count += 1;
    }
}

#[derive(Debug)]
pub struct AssemblyProfiler {
    pub instruction_counts: HashMap<String, usize>,
    pub register_usage: HashMap<u8, usize>,
    pub call_graph: HashMap<String, Vec<String>>,
    pub data_section_size: usize,
    pub code_section_size: usize,
    pub symbol_table: HashMap<String, SymbolInfo>,
}

#[derive(Debug)]
pub struct SymbolInfo {
    pub name: String,
    pub address: u64,
    pub size: u64,
    pub kind: SymbolKind,
    pub usage_count: usize,
}

#[derive(Debug)]
pub enum SymbolKind {
    Function,
    Data,
    External,
    Label,
}

impl AssemblyProfiler {
    pub fn new() -> Self {
        Self {
            instruction_counts: HashMap::new(),
            register_usage: HashMap::new(),
            call_graph: HashMap::new(),
            data_section_size: 0,
            code_section_size: 0,
            symbol_table: HashMap::new(),
        }
    }
    
    pub fn analyze_program(&mut self, program: &crate::program::Program) -> ProfileReport {
        // Analyze the program and generate a report
        let mut report = ProfileReport::new();
        
        // For now, just create a basic report since we don't have access to instructions
        report.add_insight("Program analysis completed".to_string());
        report.add_insight("Bytecode size: ".to_string() + &program.emit_bytecode().len().to_string());
        
        report
    }
}

#[derive(Debug)]
pub struct ProfileReport {
    pub insights: Vec<String>,
    pub recommendations: Vec<String>,
    pub performance_score: f64,
}

impl ProfileReport {
    pub fn new() -> Self {
        Self {
            insights: Vec::new(),
            recommendations: Vec::new(),
            performance_score: 0.0,
        }
    }
    
    pub fn add_insight(&mut self, insight: String) {
        self.insights.push(insight);
    }
    
    pub fn add_recommendation(&mut self, recommendation: String) {
        self.recommendations.push(recommendation);
    }
    
    pub fn print_report(&self) {
        println!("📊 Assembly Profile Report");
        println!("═══════════════════════════════════════");
        
        println!("🔍 Insights:");
        for insight in &self.insights {
            println!("  • {}", insight);
        }
        
        if !self.recommendations.is_empty() {
            println!("");
            println!("💡 Recommendations:");
            for recommendation in &self.recommendations {
                println!("  • {}", recommendation);
            }
        }
        
        println!("");
        println!("📈 Performance Score: {:.1}/10", self.performance_score);
        println!("═══════════════════════════════════════");
    }
} 