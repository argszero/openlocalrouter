use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Provider 预设模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPreset {
    pub name: String,
    pub base_url: String,
    pub api_types: Vec<String>,
    #[serde(default)]
    pub api_urls: Option<HashMap<String, String>>,
    pub icon: String,
    pub category: String,
    pub description: String,
    pub models_hint: Option<Vec<PresetModelHint>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetModelHint {
    pub slug: String,
    pub display_name: String,
    pub context_window: Option<i64>,
}

/// 获取所有可用的 Provider 预设
pub fn get_presets() -> Vec<ProviderPreset> {
    vec![
        ProviderPreset {
            name: "OpenAI".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_types: vec!["openai_chat".into(), "openai_responses".into()],
            api_urls: None,
            icon: "🤖".into(),
            category: "official".into(),
            description: "OpenAI 官方 API，支持 GPT-4o、GPT-5 等模型".into(),
            models_hint: Some(vec![
                PresetModelHint {
                    slug: "gpt-4o".into(),
                    display_name: "GPT-4o".into(),
                    context_window: Some(128_000),
                },
                PresetModelHint {
                    slug: "gpt-4o-mini".into(),
                    display_name: "GPT-4o Mini".into(),
                    context_window: Some(128_000),
                },
                PresetModelHint {
                    slug: "gpt-5".into(),
                    display_name: "GPT-5".into(),
                    context_window: Some(128_000),
                },
                PresetModelHint {
                    slug: "o3".into(),
                    display_name: "o3".into(),
                    context_window: Some(200_000),
                },
            ]),
        },
        ProviderPreset {
            name: "Anthropic".into(),
            base_url: "https://api.anthropic.com/v1".into(),
            api_types: vec!["anthropic_messages".into()],
            api_urls: None,
            icon: "🧠".into(),
            category: "official".into(),
            description: "Anthropic 官方 API，Claude 系列模型".into(),
            models_hint: Some(vec![
                PresetModelHint {
                    slug: "claude-sonnet-5".into(),
                    display_name: "Claude Sonnet 5".into(),
                    context_window: Some(200_000),
                },
                PresetModelHint {
                    slug: "claude-haiku-4-5".into(),
                    display_name: "Claude Haiku 4.5".into(),
                    context_window: Some(200_000),
                },
                PresetModelHint {
                    slug: "claude-opus-4-8".into(),
                    display_name: "Claude Opus 4.8".into(),
                    context_window: Some(200_000),
                },
            ]),
        },
        ProviderPreset {
            name: "Google Gemini".into(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
            api_types: vec!["openai_chat".into()],
            api_urls: None,
            icon: "🌐".into(),
            category: "official".into(),
            description: "Google Gemini API，通过 OpenAI 兼容接口访问".into(),
            models_hint: Some(vec![
                PresetModelHint {
                    slug: "gemini-2.5-pro".into(),
                    display_name: "Gemini 2.5 Pro".into(),
                    context_window: Some(2_097_152),
                },
                PresetModelHint {
                    slug: "gemini-2.5-flash".into(),
                    display_name: "Gemini 2.5 Flash".into(),
                    context_window: Some(1_048_576),
                },
            ]),
        },
        ProviderPreset {
            name: "Groq".into(),
            base_url: "https://api.groq.com/openai/v1".into(),
            api_types: vec!["openai_chat".into()],
            api_urls: None,
            icon: "🚀".into(),
            category: "cloud".into(),
            description: "Groq Cloud，超低延迟推理，OpenAI 兼容接口".into(),
            models_hint: Some(vec![
                PresetModelHint {
                    slug: "llama-4-scout".into(),
                    display_name: "Llama 4 Scout".into(),
                    context_window: Some(131_072),
                },
                PresetModelHint {
                    slug: "mixtral-8x7b".into(),
                    display_name: "Mixtral 8x7B".into(),
                    context_window: Some(32_768),
                },
            ]),
        },
        ProviderPreset {
            name: "硅基流动 (SiliconFlow)".into(),
            base_url: "https://api.siliconflow.cn/v1".into(),
            api_types: vec!["openai_chat".into()],
            api_urls: None,
            icon: "⚡".into(),
            category: "cloud".into(),
            description: "硅基流动 SiliconFlow，国产模型推理平台，OpenAI 兼容接口".into(),
            models_hint: Some(vec![
                PresetModelHint {
                    slug: "Qwen/Qwen3-235B-A22B".into(),
                    display_name: "Qwen3-235B-A22B".into(),
                    context_window: Some(131_072),
                },
                PresetModelHint {
                    slug: "deepseek-ai/DeepSeek-V3".into(),
                    display_name: "DeepSeek-V3".into(),
                    context_window: Some(65_536),
                },
                PresetModelHint {
                    slug: "deepseek-ai/DeepSeek-R1".into(),
                    display_name: "DeepSeek-R1".into(),
                    context_window: Some(65_536),
                },
                PresetModelHint {
                    slug: "Pro/Qwen/Qwen3-235B-A22B-Thinking".into(),
                    display_name: "Qwen3-235B-A22B-Thinking".into(),
                    context_window: Some(131_072),
                },
            ]),
        },
        ProviderPreset {
            name: "阿里云 TokenPlan".into(),
            base_url: "https://token-plan.cn-beijing.maas.aliyuncs.com/v1".into(),
            api_types: vec![
                "openai_chat".into(),
                "openai_responses".into(),
                "anthropic_messages".into(),
            ],
            api_urls: Some(HashMap::from([
                (
                    "openai_chat".into(),
                    "https://token-plan.cn-beijing.maas.aliyuncs.com/v1".into(),
                ),
                (
                    "openai_responses".into(),
                    "https://token-plan.cn-beijing.maas.aliyuncs.com/v1".into(),
                ),
                (
                    "anthropic_messages".into(),
                    "https://token-plan.cn-beijing.maas.aliyuncs.com/apps/anthropic".into(),
                ),
            ])),
            icon: "☁️".into(),
            category: "cloud".into(),
            description: "阿里云 TokenPlan，支持 OpenAI Chat/Responses 和 Anthropic Messages 协议"
                .into(),
            models_hint: None,
        },
        ProviderPreset {
            name: "DeepSeek".into(),
            base_url: "https://api.deepseek.com/v1".into(),
            api_types: vec!["openai_chat".into(), "anthropic_messages".into()],
            api_urls: Some(HashMap::from([
                ("openai_chat".into(), "https://api.deepseek.com/v1".into()),
                (
                    "anthropic_messages".into(),
                    "https://api.deepseek.com/anthropic".into(),
                ),
            ])),
            icon: "🔍".into(),
            category: "cloud".into(),
            description: "DeepSeek API，同时支持 OpenAI Chat 和 Anthropic Messages 协议".into(),
            models_hint: None,
        },
        ProviderPreset {
            name: "OpenRouter".into(),
            base_url: "https://openrouter.ai/api/v1".into(),
            api_types: vec!["openai_chat".into(), "anthropic_messages".into()],
            api_urls: None,
            icon: "🔀".into(),
            category: "cloud".into(),
            description: "OpenRouter 聚合网关，支持 OpenAI Chat 和 Anthropic Messages 协议".into(),
            models_hint: None,
        },
        ProviderPreset {
            name: "Ollama".into(),
            base_url: "http://localhost:11434/v1".into(),
            api_types: vec!["openai_chat".into()],
            api_urls: None,
            icon: "🦙".into(),
            category: "custom".into(),
            description: "本地 Ollama 实例，OpenAI 兼容接口".into(),
            models_hint: None,
        },
        ProviderPreset {
            name: "自定义".into(),
            base_url: String::new(),
            api_types: vec!["openai_chat".into()],
            api_urls: None,
            icon: "⚙️".into(),
            category: "custom".into(),
            description: "手动填写所有参数".into(),
            models_hint: None,
        },
    ]
}
