use crate::config::{PatternConfig, PatternType};
use glob::Pattern as GlobPattern;
use regex::Regex;

/// Pattern matcher that supports exact, glob, and regex matching
#[derive(Debug, Clone)]
pub enum PatternMatcher {
    Exact(String),
    Glob(GlobPattern),
    Regex(Regex),
}

impl PatternMatcher {
    /// Create a PatternMatcher from pattern and pattern type
    pub fn new(pattern: String, pattern_type: PatternType) -> Result<Self, String> {
        let config = PatternConfig { pattern, pattern_type };
        Self::from_config(&config)
    }
    
    /// Create a PatternMatcher from a PatternConfig
    pub fn from_config(config: &PatternConfig) -> Result<Self, String> {
        match config.pattern_type {
            PatternType::Exact => Ok(PatternMatcher::Exact(config.pattern.clone())),
            PatternType::Glob => {
                let pattern = GlobPattern::new(&config.pattern)
                    .map_err(|e| format!("Invalid glob pattern: {}", e))?;
                Ok(PatternMatcher::Glob(pattern))
            }
            PatternType::Regex => {
                let regex = Regex::new(&config.pattern)
                    .map_err(|e| format!("Invalid regex pattern: {}", e))?;
                Ok(PatternMatcher::Regex(regex))
            }
        }
    }
    
    /// Check if the given string matches this pattern
    pub fn matches(&self, text: &str) -> bool {
        match self {
            PatternMatcher::Exact(pattern) => text == pattern,
            PatternMatcher::Glob(pattern) => pattern.matches(text),
            PatternMatcher::Regex(regex) => regex.is_match(text),
        }
    }
    
    /// Get the pattern string for debugging/logging
    pub fn pattern(&self) -> &str {
        match self {
            PatternMatcher::Exact(s) => s,
            PatternMatcher::Glob(p) => p.as_str(),
            PatternMatcher::Regex(r) => r.as_str(),
        }
    }
    
    /// Get the pattern type for debugging/logging
    #[allow(dead_code)]
    pub fn pattern_type(&self) -> &PatternType {
        match self {
            PatternMatcher::Exact(_) => &PatternType::Exact,
            PatternMatcher::Glob(_) => &PatternType::Glob,
            PatternMatcher::Regex(_) => &PatternType::Regex,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_exact_match() {
        let config = PatternConfig {
            pattern: "test".to_string(),
            pattern_type: PatternType::Exact,
        };
        let matcher = PatternMatcher::from_config(&config).unwrap();
        
        assert!(matcher.matches("test"));
        assert!(!matcher.matches("test2"));
        assert!(!matcher.matches("tes"));
    }
    
    #[test]
    fn test_glob_match() {
        let config = PatternConfig {
            pattern: "/usr/bin/*".to_string(),
            pattern_type: PatternType::Glob,
        };
        let matcher = PatternMatcher::from_config(&config).unwrap();
        
        assert!(matcher.matches("/usr/bin/curl"));
        assert!(matcher.matches("/usr/bin/wget"));
        assert!(!matcher.matches("/usr/local/bin/curl"));
    }
    
    #[test]
    fn test_regex_match() {
        let config = PatternConfig {
            pattern: r"^/usr/bin/.*$".to_string(),
            pattern_type: PatternType::Regex,
        };
        let matcher = PatternMatcher::from_config(&config).unwrap();
        
        assert!(matcher.matches("/usr/bin/curl"));
        assert!(matcher.matches("/usr/bin/wget"));
        assert!(!matcher.matches("/usr/local/bin/curl"));
    }
    
    #[test]
    fn test_invalid_regex() {
        let config = PatternConfig {
            pattern: "[invalid".to_string(),
            pattern_type: PatternType::Regex,
        };
        
        assert!(PatternMatcher::from_config(&config).is_err());
    }
}
