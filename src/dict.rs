//! Dictionary encoding — maps common prediction types and concepts to compact u32 codes.
//!
//! Instead of sending strings like "code_generation" over the wire (12+ bytes),
//! we send a u32 code (4 bytes). The dictionary is synchronized across the network.

/// Dictionary of common prediction types.
/// The command brain uses these to tell regions what to prepare for.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredictionCode {
    /// No prediction / unknown
    Unknown = 0,
    /// User is asking for code generation
    CodeGeneration = 1,
    /// Mathematical reasoning / proof
    MathReasoning = 2,
    /// General text completion / writing
    TextCompletion = 3,
    /// Visual processing (image analysis)
    VisualProcessing = 4,
    /// Audio processing (speech, music)
    AudioProcessing = 5,
    /// Action / API call planning
    ActionPlanning = 6,
    /// Memory recall (fact retrieval)
    MemoryRecall = 7,
    /// Scientific reasoning
    ScientificReasoning = 8,
    /// Logical / analytical reasoning
    LogicalReasoning = 9,
    /// Translation between languages
    Translation = 10,
    /// Summarization of long text
    Summarization = 11,
    /// Question answering
    QuestionAnswering = 12,
    /// Creative writing / generation
    CreativeWriting = 13,
    /// Code debugging / analysis
    CodeDebugging = 14,
    /// Data analysis / statistics
    DataAnalysis = 15,
    /// Planning / task decomposition
    TaskPlanning = 16,
    /// Tool use / API orchestration
    ToolUse = 17,
    /// Multi-modal integration
    Multimodal = 18,
    /// Long-term memory consolidation
    MemoryConsolidation = 19,
    /// Self-reflection / self-critique
    SelfReflection = 20,
    /// Security / safety check
    SafetyCheck = 21,
    /// Emotional / social reasoning
    SocialReasoning = 22,
    /// Physical world simulation
    PhysicsSimulation = 23,
    /// Learning / training mode
    Learning = 24,
}

impl PredictionCode {
    /// Convert u32 to PredictionCode
    #[inline]
    pub fn from_u32(code: u32) -> PredictionCode {
        match code {
            1 => PredictionCode::CodeGeneration,
            2 => PredictionCode::MathReasoning,
            3 => PredictionCode::TextCompletion,
            4 => PredictionCode::VisualProcessing,
            5 => PredictionCode::AudioProcessing,
            6 => PredictionCode::ActionPlanning,
            7 => PredictionCode::MemoryRecall,
            8 => PredictionCode::ScientificReasoning,
            9 => PredictionCode::LogicalReasoning,
            10 => PredictionCode::Translation,
            11 => PredictionCode::Summarization,
            12 => PredictionCode::QuestionAnswering,
            13 => PredictionCode::CreativeWriting,
            14 => PredictionCode::CodeDebugging,
            15 => PredictionCode::DataAnalysis,
            16 => PredictionCode::TaskPlanning,
            17 => PredictionCode::ToolUse,
            18 => PredictionCode::Multimodal,
            19 => PredictionCode::MemoryConsolidation,
            20 => PredictionCode::SelfReflection,
            21 => PredictionCode::SafetyCheck,
            22 => PredictionCode::SocialReasoning,
            23 => PredictionCode::PhysicsSimulation,
            24 => PredictionCode::Learning,
            _ => PredictionCode::Unknown,
        }
    }

    /// Get the human-readable name
    #[inline]
    pub fn name(&self) -> &'static str {
        match self {
            PredictionCode::Unknown => "unknown",
            PredictionCode::CodeGeneration => "code_generation",
            PredictionCode::MathReasoning => "math_reasoning",
            PredictionCode::TextCompletion => "text_completion",
            PredictionCode::VisualProcessing => "visual_processing",
            PredictionCode::AudioProcessing => "audio_processing",
            PredictionCode::ActionPlanning => "action_planning",
            PredictionCode::MemoryRecall => "memory_recall",
            PredictionCode::ScientificReasoning => "scientific_reasoning",
            PredictionCode::LogicalReasoning => "logical_reasoning",
            PredictionCode::Translation => "translation",
            PredictionCode::Summarization => "summarization",
            PredictionCode::QuestionAnswering => "question_answering",
            PredictionCode::CreativeWriting => "creative_writing",
            PredictionCode::CodeDebugging => "code_debugging",
            PredictionCode::DataAnalysis => "data_analysis",
            PredictionCode::TaskPlanning => "task_planning",
            PredictionCode::ToolUse => "tool_use",
            PredictionCode::Multimodal => "multimodal",
            PredictionCode::MemoryConsolidation => "memory_consolidation",
            PredictionCode::SelfReflection => "self_reflection",
            PredictionCode::SafetyCheck => "safety_check",
            PredictionCode::SocialReasoning => "social_reasoning",
            PredictionCode::PhysicsSimulation => "physics_simulation",
            PredictionCode::Learning => "learning",
        }
    }
}

/// Region mask constants — which brain region(s) to target with a command
pub mod regions {
    /// Bit position for each brain region in the target_mask field
    pub const SENSORY: u32 = 1 << 0;
    pub const LANGUAGE: u32 = 1 << 1;
    pub const REASONING: u32 = 1 << 2;
    pub const MEMORY: u32 = 1 << 3;
    pub const MOTOR: u32 = 1 << 4;
    pub const VISUAL: u32 = 1 << 5;
    pub const AUDIO: u32 = 1 << 6;
    pub const EXECUTIVE: u32 = 1 << 7;
    pub const EMOTION: u32 = 1 << 8;
    pub const ALL: u32 = 0xFFFFFFFF;
}

// Byte-size names for common strings (alternative to u32 codes)
// Used when human-readable names are needed in debug/telemetry
pub mod compact_names {
    /// Encode a common neuron name to a u16 code
    pub fn name_to_code(name: &str) -> Option<u16> {
        match name {
            "sensory" => Some(1),
            "language" => Some(2),
            "reasoning" => Some(3),
            "memory" => Some(4),
            "motor" => Some(5),
            "visual" => Some(6),
            "audio" => Some(7),
            "executive" => Some(8),
            "command_brain" => Some(9),
            _ => None,
        }
    }

    /// Decode a u16 code back to a name
    pub fn code_to_name(code: u16) -> &'static str {
        match code {
            1 => "sensory",
            2 => "language",
            3 => "reasoning",
            4 => "memory",
            5 => "motor",
            6 => "visual",
            7 => "audio",
            8 => "executive",
            9 => "command_brain",
            _ => "unknown",
        }
    }
}
