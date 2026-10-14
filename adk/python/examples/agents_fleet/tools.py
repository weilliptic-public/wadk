import os
import re
import json
import math
import shutil
import socket
import platform
import subprocess
import urllib.request
import urllib.parse
from pathlib import Path
from typing import Optional

from langchain_core.tools import tool


# ============================================================================
# 1. CALCULATOR
# ============================================================================

@tool
def calculator(expression: str) -> str:
    """
    Evaluate a mathematical expression.

    Supports arithmetic, percentages, powers, square roots and common
    mathematical functions.

    Examples:
        "15% of 8500"
        "(1250 * 1.18) / 5"
        "sqrt(144)"
        "2 ** 10"
    """
    try:
        expression = expression.strip()

        # Simple percentage support: "15% of 8500"
        percentage_match = re.fullmatch(
            r"([\d.]+)%\s+of\s+([\d.]+)",
            expression,
            re.IGNORECASE,
        )

        if percentage_match:
            percentage = float(percentage_match.group(1))
            value = float(percentage_match.group(2))
            return str(percentage / 100 * value)

        allowed_names = {
            "sqrt": math.sqrt,
            "pow": pow,
            "abs": abs,
            "round": round,
            "ceil": math.ceil,
            "floor": math.floor,
            "sin": math.sin,
            "cos": math.cos,
            "tan": math.tan,
            "log": math.log,
            "log10": math.log10,
            "pi": math.pi,
            "e": math.e,
        }

        # Restrict evaluation to known math functions.
        result = eval(
            expression,
            {"__builtins__": {}},
            allowed_names,
        )

        return json.dumps({
            "expression": expression,
            "result": result,
        })

    except Exception as e:
        return json.dumps({
            "error": f"Invalid mathematical expression: {e}"
        })


# ============================================================================
# 2. WEB FETCH
# ============================================================================

@tool
def web_fetch(url: str) -> str:
    """
    Fetch the contents of a public HTTP/HTTPS URL.

    Useful for reading documentation, public webpages and APIs.
    """
    try:
        parsed = urllib.parse.urlparse(url)

        if parsed.scheme not in ("http", "https"):
            return json.dumps({
                "error": "Only HTTP and HTTPS URLs are allowed."
            })

        request = urllib.request.Request(
            url,
            headers={
                "User-Agent": "LangChain-Agent/1.0"
            },
        )

        with urllib.request.urlopen(request, timeout=10) as response:
            content = response.read(100_000).decode(
                "utf-8",
                errors="replace",
            )

        return json.dumps({
            "url": url,
            "status": response.status,
            "content": content[:20_000],
        })

    except Exception as e:
        return json.dumps({
            "url": url,
            "error": str(e),
        })


# ============================================================================
# 3. FILE SEARCH
# ============================================================================

@tool
def file_search(
    directory: str,
    query: str,
    extension: Optional[str] = None,
) -> str:
    """
    Search files under a directory for a text query.

    Args:
        directory: Directory to search.
        query: Text to search for.
        extension: Optional file extension such as ".py" or ".json".
    """
    try:
        root = Path(directory).expanduser().resolve()

        if not root.exists():
            return json.dumps({
                "error": f"Directory does not exist: {root}"
            })

        matches = []

        for path in root.rglob("*"):
            if not path.is_file():
                continue

            if extension and path.suffix != extension:
                continue

            try:
                text = path.read_text(
                    encoding="utf-8",
                    errors="ignore",
                )

                if query.lower() in text.lower():
                    matches.append(str(path))

            except (PermissionError, OSError):
                continue

            if len(matches) >= 50:
                break

        return json.dumps({
            "directory": str(root),
            "query": query,
            "matches": matches,
            "count": len(matches),
        })

    except Exception as e:
        return json.dumps({
            "error": str(e),
        })


# ============================================================================
# 4. SYSTEM INFO
# ============================================================================

@tool
def system_info() -> str:
    """
    Get information about the current operating system and machine.
    """
    try:
        total_memory = None
        available_memory = None

        # Linux/macOS friendly approach where possible.
        if hasattr(os, "sysconf"):
            try:
                page_size = os.sysconf("SC_PAGE_SIZE")
                total_pages = os.sysconf("SC_PHYS_PAGES")
                available_pages = os.sysconf("SC_AVPHYS_PAGES")

                total_memory = page_size * total_pages
                available_memory = page_size * available_pages
            except (ValueError, OSError):
                pass

        return json.dumps({
            "hostname": socket.gethostname(),
            "os": platform.system(),
            "os_version": platform.version(),
            "architecture": platform.machine(),
            "processor": platform.processor(),
            "python_version": platform.python_version(),
            "cpu_count": os.cpu_count(),
            "memory_total_bytes": total_memory,
            "memory_available_bytes": available_memory,
        }, indent=2)

    except Exception as e:
        return json.dumps({
            "error": str(e),
        })


# ============================================================================
# 5. PROCESS MANAGER
# ============================================================================

@tool
def process_manager(search: Optional[str] = None) -> str:
    """
    List running processes.

    Optionally search for processes by name.

    Args:
        search: Optional process name to search for.
    """
    try:
        system = platform.system()

        if system == "Windows":
            command = [
                "tasklist",
                "/FO",
                "CSV",
                "/NH",
            ]
        else:
            command = [
                "ps",
                "-eo",
                "pid,ppid,%cpu,%mem,comm",
            ]

        result = subprocess.run(
            command,
            capture_output=True,
            text=True,
            timeout=10,
        )

        output = result.stdout

        if search:
            lines = output.splitlines()

            lines = [
                line
                for line in lines
                if search.lower() in line.lower()
            ]

            output = "\n".join(lines)

        return json.dumps({
            "search": search,
            "processes": output[:20_000],
        })

    except Exception as e:
        return json.dumps({
            "error": str(e),
        })


# ============================================================================
# 6. DISK ANALYZER
# ============================================================================

@tool
def disk_analyzer(
    path: str = ".",
    top_n: int = 10,
) -> str:
    """
    Analyze disk usage for a directory.

    Returns filesystem capacity and the largest immediate children.
    """
    try:
        root = Path(path).expanduser().resolve()

        if not root.exists():
            return json.dumps({
                "error": f"Path does not exist: {root}"
            })

        usage = shutil.disk_usage(root)

        children = []

        for child in root.iterdir():
            try:
                if child.is_file():
                    size = child.stat().st_size

                elif child.is_dir():
                    size = 0

                    for item in child.rglob("*"):
                        try:
                            if item.is_file():
                                size += item.stat().st_size
                        except (PermissionError, OSError):
                            pass

                else:
                    continue

                children.append({
                    "path": str(child),
                    "size_bytes": size,
                })

            except (PermissionError, OSError):
                continue

        children.sort(
            key=lambda x: x["size_bytes"],
            reverse=True,
        )

        return json.dumps({
            "path": str(root),
            "total_bytes": usage.total,
            "used_bytes": usage.used,
            "free_bytes": usage.free,
            "largest_items": children[:top_n],
        }, indent=2)

    except Exception as e:
        return json.dumps({
            "error": str(e),
        })


# ============================================================================
# 7. NETWORK DIAGNOSTICS
# ============================================================================

@tool
def network_diagnostics(
    hostname: str,
) -> str:
    """
    Perform basic network diagnostics for a hostname.

    Resolves DNS and attempts a TCP connection to common ports.
    """
    result = {
        "hostname": hostname,
        "dns": {},
        "ports": {},
    }

    try:
        addresses = socket.getaddrinfo(
            hostname,
            None,
            socket.AF_UNSPEC,
        )

        ips = sorted({
            address[4][0]
            for address in addresses
        })

        result["dns"]["resolved"] = True
        result["dns"]["addresses"] = ips

    except socket.gaierror as e:
        result["dns"] = {
            "resolved": False,
            "error": str(e),
        }

        return json.dumps(result, indent=2)

    # Check common ports.
    for port in [22, 80, 443, 8080]:
        try:
            with socket.create_connection(
                (hostname, port),
                timeout=2,
            ):
                result["ports"][str(port)] = "open"

        except (TimeoutError, OSError):
            result["ports"][str(port)] = "closed/unreachable"

    return json.dumps(result, indent=2)


# ============================================================================
# 8. ENVIRONMENT VARIABLES
# ============================================================================

@tool
def environment_variables(
    prefix: Optional[str] = None,
) -> str:
    """
    Inspect environment variable names.

    By default, values are masked to avoid exposing secrets.

    Args:
        prefix: Optional prefix such as "OPENAI" or "PATH".
    """
    try:
        variables = {}

        for key, value in os.environ.items():

            if prefix and not key.startswith(prefix):
                continue

            # Don't expose likely secrets.
            sensitive = any(
                word in key.upper()
                for word in [
                    "KEY",
                    "SECRET",
                    "TOKEN",
                    "PASSWORD",
                    "CREDENTIAL",
                ]
            )

            variables[key] = (
                "***MASKED***"
                if sensitive
                else value
            )

        return json.dumps({
            "count": len(variables),
            "variables": variables,
        }, indent=2)

    except Exception as e:
        return json.dumps({
            "error": str(e),
        })


# ============================================================================
# 9. PORT CHECKER
# ============================================================================

@tool
def port_checker(
    host: str,
    port: int,
    timeout: float = 2.0,
) -> str:
    """
    Check whether a TCP port is reachable on a host.

    Examples:
        localhost:3000
        localhost:8000
        example.com:443
    """
    try:
        with socket.create_connection(
            (host, port),
            timeout=timeout,
        ):
            return json.dumps({
                "host": host,
                "port": port,
                "reachable": True,
            })

    except Exception as e:
        return json.dumps({
            "host": host,
            "port": port,
            "reachable": False,
            "error": str(e),
        })


# ============================================================================
# 10. SAFE COMMAND EXECUTION
# ============================================================================

ALLOWED_COMMANDS = {
    "pwd",
    "ls",
    "whoami",
    "date",
    "uname",
    "hostname",
    "uptime",
    "df",
    "du",
    "python",
}

@tool
def safe_command_execution(
    command: str,
) -> str:
    """
    Execute a restricted set of safe local commands.

    Only commands from the predefined allowlist are permitted.
    """
    try:
        parts = command.strip().split()

        if not parts:
            return json.dumps({
                "error": "Empty command."
            })

        executable = parts[0]

        if executable not in ALLOWED_COMMANDS:
            return json.dumps({
                "error": f"Command '{executable}' is not allowed.",
                "allowed_commands": sorted(ALLOWED_COMMANDS),
            })

        result = subprocess.run(
            parts,
            capture_output=True,
            text=True,
            timeout=10,
        )

        return json.dumps({
            "command": command,
            "return_code": result.returncode,
            "stdout": result.stdout[:10_000],
            "stderr": result.stderr[:5_000],
        })

    except subprocess.TimeoutExpired:
        return json.dumps({
            "error": "Command timed out."
        })

    except Exception as e:
        return json.dumps({
            "error": str(e),
        })


# ============================================================================
# TOOL COLLECTION
# ============================================================================

SYSTEM_TOOLS = [
    calculator,
    web_fetch,
    file_search,
    system_info,
    process_manager,
    disk_analyzer,
    network_diagnostics,
    environment_variables,
    port_checker,
    safe_command_execution,
]