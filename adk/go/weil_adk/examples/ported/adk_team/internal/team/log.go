package team

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"sync/atomic"
	"time"
)

// Log levels, as Python's logging.
const (
	DEBUG    = 10
	INFO     = 20
	WARNING  = 30
	ERROR    = 40
	CRITICAL = 50
)

var logLevel atomic.Int32

func init() { logLevel.Store(WARNING) }

// SetupLogger is google.adk.cli.utils.logs.setup_adk_logger(level): records
// at level or above go to stderr as
// "<time> - <LEVEL> - <file>:<line> - <message>".
func SetupLogger(level int) { logLevel.Store(int32(level)) }

// Logger is logging.getLogger("google_adk." + module).
type Logger struct{}

func (Logger) Debugf(format string, args ...any)    { logf(DEBUG, "DEBUG", format, args...) }
func (Logger) Infof(format string, args ...any)     { logf(INFO, "INFO", format, args...) }
func (Logger) Warningf(format string, args ...any)  { logf(WARNING, "WARNING", format, args...) }
func (Logger) Errorf(format string, args ...any)    { logf(ERROR, "ERROR", format, args...) }
func (Logger) Criticalf(format string, args ...any) { logf(CRITICAL, "CRITICAL", format, args...) }

// Log is the shared logger.
var Log Logger

func logf(level int, name, format string, args ...any) {
	if int32(level) < logLevel.Load() {
		return
	}
	_, file, line, _ := runtime.Caller(2)
	now := time.Now()
	fmt.Fprintf(os.Stderr, "%s,%03d - %s - %s:%d - %s\n",
		now.Format("2006-01-02 15:04:05"), now.Nanosecond()/1e6, name,
		filepath.Base(file), line, fmt.Sprintf(format, args...))
}

// Fatalf prints the message to stderr and exits 1, as an uncaught Python
// exception ends the script.
func Fatalf(format string, args ...any) {
	fmt.Fprintf(os.Stderr, format+"\n", args...)
	os.Exit(1)
}
