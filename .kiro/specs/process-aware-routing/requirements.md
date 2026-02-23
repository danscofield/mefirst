# Requirements Document

## Introduction

This feature extends the existing eBPF-based proxy system to capture process metadata at connection time and enable process-aware routing decisions. When a connection is accepted by the proxy, the system will identify the originating process (uid, username, pid, executable path, command line arguments) by spidering the /proc filesystem. The proxy will log this metadata and use it to apply routing rules based on process identity, executable patterns, command line patterns, and HTTP Host header patterns.

The eBPF component continues to handle connection interception and redirection, while process metadata retrieval is performed entirely in userspace using /proc filesystem access. This approach simplifies the implementation and avoids eBPF verifier complexity while still providing full process identification capabilities.

## Glossary

- **eBPF_Hook**: The kernel-space eBPF program attached to the cgroup connect4 hook that intercepts TCP connection attempts and redirects them to the proxy
- **Process_Metadata**: A data structure containing uid, username, pid, executable file path, and command line arguments for a process
- **Proc_Spider**: The userspace component that retrieves process metadata by reading /proc filesystem entries
- **Socket_Inode**: A unique identifier for a socket that can be used to find which process owns it by searching /proc/*/fd/*
- **Connection_Interception_Config**: Configuration specifying which connections to intercept, consisting of an optional IP address and a required port number
- **Proxy_Handler**: The userspace Rust component that handles HTTP requests and applies routing rules
- **Routing_Rule**: A configuration entry that specifies conditions under which a proxy plugin should be applied
- **Pattern_Matcher**: A component that evaluates patterns (exact, glob, or regex) against strings
- **Host_Header**: The HTTP Host header field that identifies the target hostname
- **LSM_Hook**: A Linux Security Module hook that restricts ptrace operations to read-only mode for the proxy process

## Requirements

### Requirement 1: Capture Process Metadata via Proc Spidering

**User Story:** As a system administrator, I want the proxy to identify which processes are making network requests by reading /proc filesystem entries, so that I can audit and route based on process identity.

#### Acceptance Criteria

1. WHEN the Proxy_Handler accepts a connection, THE Proc_Spider SHALL retrieve the socket inode from the accepted file descriptor
2. WHEN a socket inode is retrieved, THE Proc_Spider SHALL search /proc/*/fd/* to find which PID owns that socket
3. WHEN a PID is identified, THE Proc_Spider SHALL read the uid from /proc/<pid>/status
4. WHEN a PID is identified, THE Proc_Spider SHALL read the executable path from /proc/<pid>/exe
5. WHEN a PID is identified, THE Proc_Spider SHALL read the command line arguments from /proc/<pid>/cmdline
6. WHEN a uid is retrieved, THE Proc_Spider SHALL resolve the uid to a username using libc::getpwuid()
7. FOR ALL metadata retrieval operations, THE Proc_Spider SHALL handle failures gracefully by returning None or using fallback values

### Requirement 2: Store Original Destination in eBPF

**User Story:** As a developer, I want the eBPF hook to store the original destination IP/port before redirection, so that the userspace proxy can forward requests to the intended upstream server.

#### Acceptance Criteria

1. WHEN the eBPF_Hook intercepts a connection, THE eBPF_Hook SHALL capture the original destination IP and port before redirection
2. WHEN the original destination is captured, THE eBPF_Hook SHALL store it in a PID-indexed LRU cache for later retrieval
3. THE eBPF cache SHALL be accessible from userspace for destination lookup
4. THE eBPF cache SHALL use an LRU eviction policy to manage memory when the cache is full
5. THE eBPF cache SHALL have a configurable maximum size with a reasonable default (e.g., 10,000 entries)

### Requirement 3: Retrieve Process Metadata in Userspace

**User Story:** As a proxy developer, I want to retrieve process metadata for incoming connections using /proc filesystem access, so that I can log and route based on process identity.

#### Acceptance Criteria

1. WHEN the Proxy_Handler accepts a connection, THE Proxy_Handler SHALL extract the socket file descriptor
2. WHEN a socket file descriptor is available, THE Proxy_Handler SHALL call the Proc_Spider to retrieve process metadata
3. IF the Proc_Spider cannot identify the process, THEN THE Proxy_Handler SHALL continue processing without Process_Metadata
4. WHEN Process_Metadata is retrieved, THE Proxy_Handler SHALL make it available to the logging and routing subsystems

### Requirement 4: Log Process Metadata

**User Story:** As a system administrator, I want process information logged for each proxied request, so that I can audit which processes are making network requests.

#### Acceptance Criteria

1. WHEN Process_Metadata is available for a request, THE Proxy_Handler SHALL log the uid
2. WHEN Process_Metadata is available for a request, THE Proxy_Handler SHALL log the username
3. WHEN Process_Metadata is available for a request, THE Proxy_Handler SHALL log the pid
4. WHEN Process_Metadata is available for a request, THE Proxy_Handler SHALL log the executable file path
5. WHEN Process_Metadata is available for a request, THE Proxy_Handler SHALL log the command line arguments
6. THE Proxy_Handler SHALL include Process_Metadata in the existing request logging format
7. WHEN Process_Metadata is not available, THE Proxy_Handler SHALL log requests without process information

### Requirement 5: Configure Process-Based Routing Rules

**User Story:** As a system administrator, I want to configure routing rules based on process attributes, so that I can apply different proxy behaviors for different processes.

#### Acceptance Criteria

1. THE PluginConfig SHALL support an optional uid filter field
2. THE PluginConfig SHALL support an optional username filter field
3. THE PluginConfig SHALL support an optional executable_pattern filter field with pattern type (exact, glob, regex)
4. THE PluginConfig SHALL support an optional cmdline_pattern filter field with pattern type (exact, glob, regex)
5. THE PluginConfig SHALL support an optional host_pattern filter field with pattern type (exact, glob, regex)
6. WHERE no process-based filters are specified, THE Routing_Rule SHALL apply to all requests matching existing criteria
7. WHERE multiple process-based filters are specified, THE Routing_Rule SHALL apply only when all specified filters match (AND logic)

### Requirement 6: Apply UID-Based Routing

**User Story:** As a system administrator, I want to apply routing rules only for specific user IDs, so that I can provide different proxy behavior per user.

#### Acceptance Criteria

1. WHEN a Routing_Rule specifies a uid filter, THE Proxy_Handler SHALL retrieve the uid from Process_Metadata
2. WHEN the uid from Process_Metadata matches the uid filter, THE Proxy_Handler SHALL consider the rule as matching
3. WHEN the uid from Process_Metadata does not match the uid filter, THE Proxy_Handler SHALL skip the rule
4. WHEN Process_Metadata is not available and a uid filter is specified, THE Proxy_Handler SHALL skip the rule

### Requirement 7: Apply Username-Based Routing

**User Story:** As a system administrator, I want to apply routing rules only for specific usernames, so that I can configure policies using human-readable identifiers.

#### Acceptance Criteria

1. WHEN a Routing_Rule specifies a username filter, THE Proxy_Handler SHALL retrieve the username from Process_Metadata
2. WHEN the username from Process_Metadata matches the username filter, THE Proxy_Handler SHALL consider the rule as matching
3. WHEN the username from Process_Metadata does not match the username filter, THE Proxy_Handler SHALL skip the rule
4. WHEN Process_Metadata is not available and a username filter is specified, THE Proxy_Handler SHALL skip the rule

### Requirement 8: Apply Executable Pattern-Based Routing

**User Story:** As a system administrator, I want to apply routing rules based on the executable file path, so that I can provide different proxy behavior for different applications.

#### Acceptance Criteria

1. WHEN a Routing_Rule specifies an executable_pattern filter, THE Proxy_Handler SHALL retrieve the executable path from Process_Metadata
2. WHEN the executable_pattern uses exact matching, THE Pattern_Matcher SHALL compare the executable path for exact equality
3. WHEN the executable_pattern uses glob matching, THE Pattern_Matcher SHALL evaluate the glob pattern against the executable path
4. WHEN the executable_pattern uses regex matching, THE Pattern_Matcher SHALL evaluate the regex pattern against the executable path
5. WHEN the executable path matches the pattern, THE Proxy_Handler SHALL consider the rule as matching
6. WHEN the executable path does not match the pattern, THE Proxy_Handler SHALL skip the rule
7. WHEN Process_Metadata is not available and an executable_pattern filter is specified, THE Proxy_Handler SHALL skip the rule

### Requirement 9: Apply Command Line Pattern-Based Routing

**User Story:** As a system administrator, I want to apply routing rules based on command line arguments, so that I can differentiate behavior based on how a program was invoked.

#### Acceptance Criteria

1. WHEN a Routing_Rule specifies a cmdline_pattern filter, THE Proxy_Handler SHALL retrieve the command line arguments from Process_Metadata
2. WHEN the cmdline_pattern uses exact matching, THE Pattern_Matcher SHALL compare the command line for exact equality
3. WHEN the cmdline_pattern uses glob matching, THE Pattern_Matcher SHALL evaluate the glob pattern against the command line
4. WHEN the cmdline_pattern uses regex matching, THE Pattern_Matcher SHALL evaluate the regex pattern against the command line
5. WHEN the command line matches the pattern, THE Proxy_Handler SHALL consider the rule as matching
6. WHEN the command line does not match the pattern, THE Proxy_Handler SHALL skip the rule
7. WHEN Process_Metadata is not available and a cmdline_pattern filter is specified, THE Proxy_Handler SHALL skip the rule

### Requirement 10: Apply HTTP Host Header Pattern-Based Routing

**User Story:** As a system administrator, I want to apply routing rules based on the HTTP Host header, so that I can provide different proxy behavior for different target hosts.

#### Acceptance Criteria

1. WHEN a Routing_Rule specifies a host_pattern filter, THE Proxy_Handler SHALL extract the Host_Header from the HTTP request
2. WHEN the host_pattern uses exact matching, THE Pattern_Matcher SHALL compare the Host_Header for exact equality
3. WHEN the host_pattern uses glob matching, THE Pattern_Matcher SHALL evaluate the glob pattern against the Host_Header
4. WHEN the host_pattern uses regex matching, THE Pattern_Matcher SHALL evaluate the regex pattern against the Host_Header
5. WHEN the Host_Header matches the pattern, THE Proxy_Handler SHALL consider the rule as matching
6. WHEN the Host_Header does not match the pattern, THE Proxy_Handler SHALL skip the rule
7. WHEN the Host_Header is not present in the request and a host_pattern filter is specified, THE Proxy_Handler SHALL skip the rule

### Requirement 11: Combine Multiple Filter Criteria

**User Story:** As a system administrator, I want to combine multiple filter criteria in a single rule, so that I can create precise routing policies.

#### Acceptance Criteria

1. WHEN a Routing_Rule specifies multiple filters, THE Proxy_Handler SHALL evaluate all specified filters
2. WHEN all specified filters match, THE Proxy_Handler SHALL apply the routing rule
3. WHEN any specified filter does not match, THE Proxy_Handler SHALL skip the routing rule
4. THE Proxy_Handler SHALL support combining uid, username, executable_pattern, cmdline_pattern, and host_pattern filters in any combination

### Requirement 12: Parse and Serialize Configuration

**User Story:** As a system administrator, I want to define process-aware routing rules in configuration files, so that I can manage proxy behavior declaratively.

#### Acceptance Criteria

1. THE Config_Parser SHALL parse uid filter fields from TOML configuration files
2. THE Config_Parser SHALL parse username filter fields from TOML configuration files
3. THE Config_Parser SHALL parse executable_pattern filter fields with pattern_type from TOML configuration files
4. THE Config_Parser SHALL parse cmdline_pattern filter fields with pattern_type from TOML configuration files
5. THE Config_Parser SHALL parse host_pattern filter fields with pattern_type from TOML configuration files
6. THE Config_Parser SHALL parse uid filter fields from YAML configuration files
7. THE Config_Parser SHALL parse username filter fields from YAML configuration files
8. THE Config_Parser SHALL parse executable_pattern filter fields with pattern_type from YAML configuration files
9. THE Config_Parser SHALL parse cmdline_pattern filter fields with pattern_type from YAML configuration files
10. THE Config_Parser SHALL parse host_pattern filter fields with pattern_type from YAML configuration files
11. WHEN a configuration file contains invalid filter specifications, THE Config_Parser SHALL return a descriptive error
12. THE Config_Serializer SHALL format PluginConfig objects with process filters back into valid TOML configuration files
13. THE Config_Serializer SHALL format PluginConfig objects with process filters back into valid YAML configuration files
14. FOR ALL valid PluginConfig objects with process filters, parsing then serializing then parsing SHALL produce an equivalent object (round-trip property)

### Requirement 13: Handle eBPF Feature Flag

**User Story:** As a developer, I want process-aware routing to work only when eBPF is enabled, so that the system degrades gracefully on systems without eBPF support.

#### Acceptance Criteria

1. WHEN enable_ebpf is false, THE Proxy_Handler SHALL not attempt to retrieve Process_Metadata from the LRU_Cache
2. WHEN enable_ebpf is false and a Routing_Rule specifies process-based filters, THE Proxy_Handler SHALL skip the rule
3. WHEN enable_ebpf is false, THE Proxy_Handler SHALL log a warning if process-based routing rules are configured
4. WHEN enable_ebpf is true, THE Proxy_Handler SHALL attempt to retrieve Process_Metadata for all connections

### Requirement 14: Validate Process-Aware Configuration

**User Story:** As a system administrator, I want configuration validation to catch errors in process-aware routing rules, so that I can identify misconfigurations before runtime.

#### Acceptance Criteria

1. WHEN a Routing_Rule specifies an executable_pattern, THE Config_Validator SHALL verify that a pattern_type is specified
2. WHEN a Routing_Rule specifies a cmdline_pattern, THE Config_Validator SHALL verify that a pattern_type is specified
3. WHEN a Routing_Rule specifies a host_pattern, THE Config_Validator SHALL verify that a pattern_type is specified
4. WHEN a Routing_Rule specifies a regex pattern_type, THE Config_Validator SHALL verify that the pattern is a valid regular expression
5. WHEN a Routing_Rule specifies both uid and username filters, THE Config_Validator SHALL accept the configuration (both filters will be applied)
6. WHEN configuration validation fails, THE Config_Validator SHALL return a descriptive error message indicating which rule and field caused the failure

### Requirement 15: Support Optional IP Field in Connection Interception Configuration

**User Story:** As a system administrator, I want to make the IP field optional in connection interception configuration, so that I can capture all outbound traffic on a specific port and filter it based on Host header patterns.

#### Acceptance Criteria

1. THE Connection_Interception_Config SHALL support an optional ip field for specifying target IP addresses
2. THE Connection_Interception_Config SHALL require a port field for specifying target ports
3. WHEN the ip field is not specified, THE eBPF_Hook SHALL intercept all connections to the specified port regardless of destination IP
4. WHEN the ip field is specified, THE eBPF_Hook SHALL intercept only connections to the specified (ip, port) pair
5. WHEN the ip field is not specified and a host_pattern filter is configured, THE Proxy_Handler SHALL evaluate the Host_Header to determine whether to apply the routing rule
6. THE Config_Parser SHALL accept connection interception configurations with only a port field specified
7. THE Config_Validator SHALL verify that at least a port field is present in connection interception configurations

### Requirement 16: Check Linux Capabilities for Process Metadata Access

**User Story:** As a system administrator, I want the system to check for required Linux capabilities at startup, so that I can be notified if the eBPF program lacks permissions to read process information.

#### Acceptance Criteria

1. WHEN the eBPF_Hook is initialized, THE eBPF_Hook SHALL check for the CAP_SYS_PTRACE capability
2. WHEN the eBPF_Hook is initialized, THE eBPF_Hook SHALL check for the CAP_DAC_READ_SEARCH capability
3. WHEN CAP_SYS_PTRACE is not available, THE eBPF_Hook SHALL log a warning indicating that process command line and executable path retrieval may fail
4. WHEN CAP_DAC_READ_SEARCH is not available, THE eBPF_Hook SHALL log a warning indicating that process information access may be restricted
5. WHEN both required capabilities are available, THE eBPF_Hook SHALL log a confirmation that process metadata access is fully enabled
6. IF either capability is missing, THEN THE eBPF_Hook SHALL continue initialization and attempt to capture available process metadata fields
7. THE eBPF_Hook SHALL handle capability check failures gracefully without preventing system startup

### Requirement 17: Restrict Ptrace Access via LSM Hook

**User Story:** As a security engineer, I want to limit the proxy process to read-only ptrace access, so that even with CAP_SYS_PTRACE the proxy cannot attach to or modify other processes.

#### Acceptance Criteria

1. WHERE CAP_SYS_PTRACE is granted to the proxy process, THE LSM_Hook SHALL be implemented to restrict ptrace operations
2. WHEN a ptrace operation in ATTACH mode is attempted from the Proxy_Handler process, THE LSM_Hook SHALL block the operation
3. WHEN a ptrace operation in read-only mode is attempted from the Proxy_Handler process, THE LSM_Hook SHALL allow the operation
4. THE LSM_Hook SHALL permit read access to /proc/pid/fd for file descriptor enumeration
5. THE LSM_Hook SHALL prevent the Proxy_Handler process from modifying memory or registers of other processes via ptrace
6. WHEN the LSM_Hook blocks a ptrace operation, THE LSM_Hook SHALL log the blocked attempt with the target pid and operation type
7. THE LSM_Hook SHALL apply restrictions only to the Proxy_Handler process and not affect other system processes

### Requirement 18: Proxy HTTP Requests with Process Metadata to Commands

**User Story:** As a system administrator, I want to send HTTP requests with injected process metadata headers to external commands, so that command-based response sources can make decisions based on the originating process.

#### Acceptance Criteria

1. THE PluginConfig SHALL support an optional proxy_request_stdin boolean configuration field
2. WHEN proxy_request_stdin is true and the response source is command-based, THE Proxy_Handler SHALL inject process metadata as HTTP headers into the request
3. WHEN proxy_request_stdin is true and the response source is file-based, THE Config_Validator SHALL return an error indicating incompatibility
4. WHEN proxy_request_stdin is true and Process_Metadata is available, THE Proxy_Handler SHALL add an X-Forwarded-Uid header with the uid value
5. WHEN proxy_request_stdin is true and Process_Metadata is available, THE Proxy_Handler SHALL add an X-Forwarded-Username header with the username value
6. WHEN proxy_request_stdin is true and Process_Metadata is available, THE Proxy_Handler SHALL add an X-Forwarded-Pid header with the pid value
7. WHEN proxy_request_stdin is true and Process_Metadata is available, THE Proxy_Handler SHALL add an X-Forwarded-Process-Name header with the executable file path
8. WHEN proxy_request_stdin is true and Process_Metadata is available, THE Proxy_Handler SHALL add an X-Forwarded-Process-Args header with the command line arguments
9. WHEN proxy_request_stdin is true and the modified HTTP request is constructed, THE Proxy_Handler SHALL send the complete HTTP request (including injected headers) to the specified command via stdin
10. WHEN proxy_request_stdin is false or not specified, THE Proxy_Handler SHALL not inject process metadata headers
11. WHEN proxy_request_stdin is true and Process_Metadata is not available, THE Proxy_Handler SHALL send the original HTTP request without injected headers to the command via stdin
