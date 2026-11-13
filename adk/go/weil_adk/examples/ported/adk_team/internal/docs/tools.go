package docs

import (
	"bytes"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"time"
	"unicode/utf8"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/tool"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

// --- path confinement -------------------------------------------------------

// realpath is os.path.realpath: absolute, symlinks resolved as far as the
// path exists, the rest cleaned lexically.
func realpath(p string) string {
	abs, err := filepath.Abs(p)
	if err != nil {
		return filepath.Clean(p)
	}
	// Resolve the longest existing prefix, then re-append the rest.
	rest := ""
	cur := abs
	for {
		if r, err := filepath.EvalSymlinks(cur); err == nil {
			if rest == "" {
				return r
			}
			return filepath.Clean(filepath.Join(r, rest))
		}
		parent := filepath.Dir(cur)
		if parent == cur {
			return abs
		}
		rest = filepath.Join(filepath.Base(cur), rest)
		cur = parent
	}
}

// resolveWithinReposDir is _resolve_within_repos_dir: path resolved, or ""
// when it leaves LOCAL_REPOS_DIR_PATH.
func resolveWithinReposDir(path string) string {
	root := realpath(LocalReposDirPath)
	resolved := realpath(path)
	if resolved == root || strings.HasPrefix(resolved, root+string(os.PathSeparator)) {
		return resolved
	}
	return ""
}

// confinePath is _confine_path.
func confinePath(path, param string) (string, map[string]any) {
	safe := resolveWithinReposDir(path)
	if safe == "" {
		return "", ErrorResponse(fmt.Sprintf(
			"Access denied: %s is outside the managed repositories directory (%s): %s",
			param, LocalReposDirPath, path))
	}
	return safe, nil
}

// --- git --------------------------------------------------------------------

// gitError is subprocess.CalledProcessError of a git command.
type gitError struct {
	args   []string
	code   int
	stderr string
}

func (e *gitError) Error() string {
	return fmt.Sprintf("Command '%s' returned non-zero exit status %d.", team.Repr(e.args), e.code)
}

// gitNotFound reports whether err is Python's FileNotFoundError: git, or
// the working directory, missing.
func gitNotFound(err error) bool {
	return errors.Is(err, exec.ErrNotFound) || errors.Is(err, fs.ErrNotExist)
}

// runGit runs git args in cwd and returns stdout; with check, a non-zero
// exit is a *gitError.
func runGit(cwd string, check bool, args ...string) (stdout string, stderr string, code int, err error) {
	cmd := exec.Command("git", args...)
	cmd.Dir = cwd
	var out, errb bytes.Buffer
	cmd.Stdout, cmd.Stderr = &out, &errb
	err = cmd.Run()
	var exitErr *exec.ExitError
	if errors.As(err, &exitErr) {
		code = exitErr.ExitCode()
		if check {
			return out.String(), errb.String(), code, &gitError{args: append([]string{"git"}, args...), code: code, stderr: errb.String()}
		}
		err = nil
	}
	if err != nil {
		if cwd != "" {
			if _, serr := os.Stat(cwd); serr != nil {
				err = serr
			}
		}
		return "", "", -1, err
	}
	return out.String(), errb.String(), code, nil
}

func findHeadCommitSHA(repoPath string) (string, error) {
	out, _, _, err := runGit(repoPath, true, "rev-parse", "HEAD")
	return strings.TrimSpace(out), err
}

func osErr(err error) string {
	var pe *fs.PathError
	if errors.As(err, &pe) {
		return fmt.Sprintf("[Errno] %v: '%s'", pe.Err, pe.Path)
	}
	return err.Error()
}

func requestError(err error) map[string]any {
	var he *team.HTTPError
	if errors.As(err, &he) {
		return ErrorResponse(fmt.Sprintf("HTTP Error: %v", err))
	}
	return ErrorResponse(fmt.Sprintf("Request Error: %v", err))
}

func getOr(m any, key string, def any) any {
	if mm := team.Map(m); mm != nil {
		if v, ok := mm[key]; ok {
			return v
		}
	}
	return def
}

// --- tools ------------------------------------------------------------------

type repoArgs struct {
	RepoOwner string `json:"repo_owner"`
	RepoName  string `json:"repo_name"`
}

// ListReleases is the list_releases tool.
func ListReleases() tool.Tool {
	return team.SecuredTool("list_releases", `Lists all releases for a repository.

This function retrieves all releases and for each one, returns its ID,
creation time, publication time, and associated tag name. It handles
pagination to ensure all releases are fetched.

Args:
    repo_owner: The name of the repository owner.
    repo_name: The name of the repository.

Returns:
    A dictionary containing the status and a list of releases.`,
		func(_ agent.Context, in repoArgs) (map[string]any, error) {
			url := fmt.Sprintf("%s/repos/%s/%s/releases?per_page=100", GitHubBaseURL, in.RepoOwner, in.RepoName)
			all, err := GetPaginatedRequest(url)
			if err != nil {
				return requestError(err), nil
			}
			releases := []any{}
			for _, r := range all {
				releases = append(releases, map[string]any{
					"id":           team.Get(r, "id"),
					"tag_name":     team.Get(r, "tag_name"),
					"created_at":   team.Get(r, "created_at"),
					"published_at": team.Get(r, "published_at"),
				})
			}
			return map[string]any{"status": "success", "releases": releases}, nil
		})
}

type compareArgs struct {
	RepoOwner string `json:"repo_owner"`
	RepoName  string `json:"repo_name"`
	StartTag  string `json:"start_tag"`
	EndTag    string `json:"end_tag"`
}

func compareURL(owner, repo, start, end string) string {
	return fmt.Sprintf("%s/repos/%s/%s/compare/%s...%s", GitHubBaseURL, owner, repo, start, end)
}

func fileEntry(f any, withPatch bool) map[string]any {
	m := map[string]any{
		"relative_path": team.Get(f, "filename"),
		"status":        team.Get(f, "status"),
		"additions":     team.Get(f, "additions"),
		"deletions":     team.Get(f, "deletions"),
		"changes":       team.Get(f, "changes"),
	}
	if withPatch {
		m["patch"] = getOr(f, "patch", "No patch available.")
	}
	return m
}

// GetChangedFilesBetweenReleases is the get_changed_files_between_releases tool.
func GetChangedFilesBetweenReleases() tool.Tool {
	return team.SecuredTool("get_changed_files_between_releases", `Gets changed files and their modifications between two release tags.

Args:
    repo_owner: The name of the repository owner.
    repo_name: The name of the repository.
    start_tag: The older tag (base) for the comparison.
    end_tag: The newer tag (head) for the comparison.

Returns:
    A dictionary containing the status and a list of changed files.
    Each file includes its name, status (added, removed, modified),
    and the patch/diff content.`,
		func(_ agent.Context, in compareArgs) (map[string]any, error) {
			data, err := GetRequest(compareURL(in.RepoOwner, in.RepoName, in.StartTag, in.EndTag), nil)
			if err != nil {
				return requestError(err), nil
			}
			files := []any{}
			for _, f := range team.List(getOr(data, "files", []any{})) {
				files = append(files, fileEntry(f, true))
			}
			return map[string]any{"status": "success", "changed_files": files}, nil
		})
}

// CloneOrPullRepo is the clone_or_pull_repo tool.
func CloneOrPullRepo() tool.Tool {
	type args struct {
		RepoOwner string `json:"repo_owner"`
		RepoName  string `json:"repo_name"`
		LocalPath string `json:"local_path"`
	}
	return team.SecuredTool("clone_or_pull_repo", `Clones a GitHub repository to a local folder using owner and repo name.

If the folder already exists and is a valid Git repository, it pulls the
latest changes instead.

Args:
    repo_owner: The username or organization that owns the repository.
    repo_name: The name of the repository.
    local_path: The local directory path where the repository should be cloned
      or updated.

Returns:
    A dictionary indicating the status of the operation, output message, and
    the head commit hash.`,
		func(_ agent.Context, in args) (map[string]any, error) {
			localPath, errResp := confinePath(in.LocalPath, "local_path")
			if errResp != nil {
				return errResp, nil
			}
			repoURL := fmt.Sprintf("git@github.com:%s/%s.git", in.RepoOwner, in.RepoName)
			unexpected := func(err error) (map[string]any, error) {
				if gitNotFound(err) {
					return ErrorResponse("Error: 'git' command not found. Is Git installed?"), nil
				}
				return ErrorResponse(fmt.Sprintf("An unexpected error occurred: %s", osErr(err))), nil
			}
			var output string
			if _, err := os.Stat(localPath); err == nil {
				if st, err := os.Stat(filepath.Join(localPath, ".git")); err == nil && st.IsDir() {
					fmt.Printf("Repository exists at '%s'. Pulling latest changes...\n", localPath)
					out, _, _, err := runGit(localPath, true, "pull")
					var ge *gitError
					if errors.As(err, &ge) {
						return ErrorResponse(fmt.Sprintf("git pull failed: %s", ge.stderr)), nil
					} else if err != nil {
						return unexpected(err)
					}
					output = strings.TrimSpace(out)
				} else {
					return ErrorResponse(fmt.Sprintf("Path '%s' exists but is not a Git repository.", localPath)), nil
				}
			} else {
				fmt.Printf("Cloning from %s/%s into '%s'...\n", in.RepoOwner, in.RepoName, localPath)
				out, _, _, err := runGit("", true, "clone", repoURL, localPath)
				var ge *gitError
				if errors.As(err, &ge) {
					return ErrorResponse(fmt.Sprintf("git clone failed: %s", ge.stderr)), nil
				} else if err != nil {
					return unexpected(err)
				}
				output = strings.TrimSpace(out)
			}
			sha, err := findHeadCommitSHA(localPath)
			if err != nil {
				return unexpected(err)
			}
			return map[string]any{"status": "success", "output": output, "head_commit_sha": sha}, nil
		})
}

// splitLines is str.splitlines(): lines end at \n, \r\n, \r, \v, \f,
// \x1c, \x1d, \x1e, \x85, U+2028 and U+2029, and a final line break
// does not start an empty line.
func splitLines(s string) []string {
	var lines []string
	start := 0
	for i := 0; i < len(s); {
		r, size := utf8.DecodeRuneInString(s[i:])
		switch r {
		case '\n', '\r', '\v', '\f', '\x1c', '\x1d', '\x1e', '\u0085', '\u2028', '\u2029':
			lines = append(lines, s[start:i])
			if r == '\r' && i+1 < len(s) && s[i+1] == '\n' {
				size++ // \r\n is one line break
			}
			start = i + size
		}
		i += size
	}
	if start < len(s) {
		lines = append(lines, s[start:])
	}
	return lines
}

// utf8DecodeError is str(e) for the UnicodeDecodeError CPython's strict
// UTF-8 decoder raises on data, or "" when data is valid UTF-8.
func utf8DecodeError(data []byte) string {
	isCont := func(b byte) bool { return b >= 0x80 && b <= 0xBF }
	fail := func(start, end int, reason string) string {
		if end-start == 1 {
			return fmt.Sprintf("'utf-8' codec can't decode byte 0x%02x in position %d: %s", data[start], start, reason)
		}
		return fmt.Sprintf("'utf-8' codec can't decode bytes in position %d-%d: %s", start, end-1, reason)
	}
	for i := 0; i < len(data); {
		c := data[i]
		var n int
		lo, hi := byte(0x80), byte(0xBF) // range of the second byte
		switch {
		case c < 0x80:
			i++
			continue
		case c >= 0xC2 && c <= 0xDF:
			n = 2
		case c >= 0xE0 && c <= 0xEF:
			n = 3
			if c == 0xE0 {
				lo = 0xA0
			} else if c == 0xED {
				hi = 0x9F
			}
		case c >= 0xF0 && c <= 0xF4:
			n = 4
			if c == 0xF0 {
				lo = 0x90
			} else if c == 0xF4 {
				hi = 0x8F
			}
		default:
			return fail(i, i+1, "invalid start byte")
		}
		for k := 1; k < n; k++ {
			if i+k >= len(data) {
				return fail(i, len(data), "unexpected end of data")
			}
			b := data[i+k]
			if (k == 1 && (b < lo || b > hi)) || (k > 1 && !isCont(b)) {
				return fail(i, i+k, "invalid continuation byte")
			}
		}
		i += n
	}
	return ""
}

// ReadLocalGitRepoFileContent is the read_local_git_repo_file_content tool.
func ReadLocalGitRepoFileContent() tool.Tool {
	type args struct {
		FilePath string `json:"file_path"`
	}
	return team.SecuredTool("read_local_git_repo_file_content", `Reads the content of a specified file in a local Git repository.

Args:
    file_path: The full, absolute path to the file.

Returns:
    A dictionary containing the status, content of the file, and the head
    commit hash.`,
		func(_ agent.Context, in args) (map[string]any, error) {
			fmt.Printf("Attempting to read file from path: %s\n", in.FilePath)
			if !filepath.IsAbs(in.FilePath) {
				return ErrorResponse(fmt.Sprintf("file_path must be an absolute path, got: %s", in.FilePath)), nil
			}
			filePath, errResp := confinePath(in.FilePath, "file_path")
			if errResp != nil {
				return errResp, nil
			}
			sha, err := findHeadCommitSHA(filepath.Dir(filePath))
			if err != nil {
				sha = "unknown"
			}
			data, err := os.ReadFile(filePath)
			if errors.Is(err, fs.ErrNotExist) {
				return ErrorResponse(fmt.Sprintf("Error: File not found at %s", filePath)), nil
			} else if err != nil {
				return ErrorResponse(fmt.Sprintf("An unexpected error occurred: %s", osErr(err))), nil
			}
			// open(file_path, "r", encoding="utf-8").read() decodes strictly.
			// Its UnicodeDecodeError is no IOError/OSError, so Python's tool
			// raises it; ADK-Go turns a tool error into {"error": str(e)}.
			if msg := utf8DecodeError(data); msg != "" {
				return nil, errors.New(msg)
			}
			lines := splitLines(string(data))
			numbered := make([]string, len(lines))
			for i, l := range lines {
				numbered[i] = fmt.Sprintf("%d: %s", i+1, l)
			}
			return map[string]any{
				"status":          "success",
				"file_path":       filePath,
				"content":         strings.Join(numbered, "\n"),
				"head_commit_sha": sha,
			}, nil
		})
}

// ListDirectoryContents is the list_directory_contents tool.
func ListDirectoryContents() tool.Tool {
	type args struct {
		DirectoryPath string `json:"directory_path"`
	}
	return team.SecuredTool("list_directory_contents", `Recursively lists all files and directories within a specified directory.

Args:
    directory_path: The full, absolute path to the directory.

Returns:
    A dictionary containing the status and a map where keys are directory
    paths relative to the initial directory_path, and values are lists of
    their contents.
    Returns an error message if the directory cannot be accessed.`,
		func(_ agent.Context, in args) (map[string]any, error) {
			fmt.Printf("Attempting to recursively list contents of directory: %s\n", in.DirectoryPath)
			dirPath, errResp := confinePath(in.DirectoryPath, "directory_path")
			if errResp != nil {
				return errResp, nil
			}
			if st, err := os.Stat(dirPath); err != nil || !st.IsDir() {
				return ErrorResponse(fmt.Sprintf("Error: Directory not found at %s", dirPath)), nil
			}
			dirMap := map[string]any{}
			// os.walk, top-down, not following symlinked directories and
			// ignoring unreadable ones; hidden entries are skipped.
			var walk func(root string)
			walk = func(root string) {
				entries, err := os.ReadDir(root)
				if err != nil {
					return
				}
				var dirs, files []string
				var recurse []string
				for _, e := range entries {
					name := e.Name()
					isDir := e.IsDir()
					if e.Type()&fs.ModeSymlink != 0 {
						if st, err := os.Stat(filepath.Join(root, name)); err == nil && st.IsDir() {
							isDir = true
						}
					}
					if isDir {
						if strings.HasPrefix(name, ".") {
							continue
						}
						dirs = append(dirs, name)
						if e.Type()&fs.ModeSymlink == 0 {
							recurse = append(recurse, name)
						}
					} else if !strings.HasPrefix(name, ".") {
						files = append(files, name)
					}
				}
				rel, _ := filepath.Rel(dirPath, root)
				contents := []any{}
				for _, n := range append(dirs, files...) {
					contents = append(contents, n)
				}
				dirMap[rel] = contents
				for _, d := range recurse {
					walk(filepath.Join(root, d))
				}
			}
			walk(dirPath)
			return map[string]any{"status": "success", "directory_path": dirPath, "directory_map": dirMap}, nil
		})
}

// SearchLocalGitRepo is the search_local_git_repo tool.
func SearchLocalGitRepo() tool.Tool {
	type args struct {
		DirectoryPath string   `json:"directory_path"`
		Pattern       string   `json:"pattern"`
		Extensions    []string `json:"extensions,omitempty"`
		IgnoredDirs   []string `json:"ignored_dirs,omitempty"`
	}
	return team.SecuredTool("search_local_git_repo", `Searches a local Git repository for a pattern.

Args:
    directory_path: The absolute path to the local Git repository.
    pattern: The search pattern (can be a simple string or regex for git
      grep).
    extensions: The list of file extensions to search, e.g. ["py", "md"]. If
      None, all extensions will be searched.
    ignored_dirs: The list of directories to ignore, e.g. ["tests"]. If None,
      no directories will be ignored.

Returns:
    A dictionary containing the status, and a list of match details (relative
    file path to the directory_path, line number, content).`,
		func(_ agent.Context, in args) (map[string]any, error) {
			exts := "None"
			if in.Extensions != nil {
				exts = team.Repr(in.Extensions)
			}
			fmt.Printf("Attempting to search for pattern: %s in directory: %s, with extensions: %s\n", in.Pattern, in.DirectoryPath, exts)
			dirPath, errResp := confinePath(in.DirectoryPath, "directory_path")
			if errResp != nil {
				return errResp, nil
			}
			cmd := []string{"grep", "-n", "-I", "-E", "--ignore-case", "-e", in.Pattern}
			var pathspecs []string
			for _, e := range in.Extensions {
				pathspecs = append(pathspecs, "*."+e)
			}
			for _, d := range in.IgnoredDirs {
				pathspecs = append(pathspecs, ":(exclude)"+d)
			}
			if len(pathspecs) > 0 {
				cmd = append(append(cmd, "--"), pathspecs...)
			}
			stdout, stderr, code, err := runGit(dirPath, false, cmd...)
			if err != nil {
				if gitNotFound(err) { // Python: FileNotFoundError, git or the directory missing
					return ErrorResponse(fmt.Sprintf("Directory not found: %s", dirPath)), nil
				}
				return ErrorResponse(fmt.Sprintf("An unexpected error occurred: %s", osErr(err))), nil
			}
			if code > 1 {
				return ErrorResponse(fmt.Sprintf("git grep failed: %s", stderr)), nil
			}
			matches := []any{}
			if stdout != "" {
				for _, line := range strings.Split(strings.TrimSpace(stdout), "\n") {
					parts := strings.SplitN(line, ":", 3)
					var n int
					if len(parts) == 3 {
						n, err = strconv.Atoi(strings.TrimSpace(parts[1]))
					}
					if len(parts) != 3 || err != nil {
						return ErrorResponse(fmt.Sprintf("Error: Failed to parse line: %s from git grep output.", line)), nil
					}
					matches = append(matches, map[string]any{
						"file_path":    parts[0],
						"line_number":  n,
						"line_content": strings.TrimSpace(parts[2]),
					})
				}
			}
			return map[string]any{"status": "success", "matches": matches}, nil
		})
}

// CreatePullRequestFromChanges is the create_pull_request_from_changes tool.
func CreatePullRequestFromChanges() tool.Tool {
	type args struct {
		RepoOwner     string            `json:"repo_owner"`
		RepoName      string            `json:"repo_name"`
		LocalPath     string            `json:"local_path"`
		BaseBranch    string            `json:"base_branch"`
		Changes       map[string]string `json:"changes"`
		CommitMessage string            `json:"commit_message"`
		PRTitle       string            `json:"pr_title"`
		PRBody        string            `json:"pr_body"`
	}
	return team.SecuredTool("create_pull_request_from_changes", `Creates a new branch, applies file changes, commits, pushes, and creates a PR.

Args:
    repo_owner: The username or organization that owns the repository.
    repo_name: The name of the repository.
    local_path: The local absolute path to the cloned repository.
    base_branch: The name of the branch to merge the changes into (e.g.,
      "main").
    changes: A dictionary where keys are file paths relative to the repo root
      and values are the new and full content for those files.
    commit_message: The message for the git commit.
    pr_title: The title for the pull request.
    pr_body: The body/description for the pull request.

Returns:
    A dictionary containing the status and the pull request object on success,
    or an error message on failure.`,
		func(_ agent.Context, in args) (map[string]any, error) {
			fail := func(err error) (map[string]any, error) {
				var ge *gitError
				var he *team.HTTPError
				switch {
				case errors.As(err, &ge):
					return ErrorResponse(fmt.Sprintf("A git command failed: %s", ge.stderr)), nil
				case errors.As(err, &he):
					return ErrorResponse(fmt.Sprintf("GitHub API request failed: %v", err)), nil
				default:
					var pe *fs.PathError
					if errors.As(err, &pe) || gitNotFound(err) {
						return ErrorResponse(fmt.Sprintf("A file system error occurred: %s", osErr(err))), nil
					}
					return ErrorResponse(fmt.Sprintf("GitHub API request failed: %v", err)), nil
				}
			}
			git := func(a ...string) error {
				_, _, _, err := runGit(in.LocalPath, true, a...)
				return err
			}
			// Step 0: Ensure we are on the base branch and it's up to date.
			if err := git("checkout", in.BaseBranch); err != nil {
				return fail(err)
			}
			if err := git("pull", "origin", in.BaseBranch); err != nil {
				return fail(err)
			}
			// Step 1: Create a new, unique branch from the base branch.
			newBranch := "agent-changes-" + time.Now().Format("20060102-150405")
			if err := git("checkout", "-b", newBranch); err != nil {
				return fail(err)
			}
			fmt.Printf("Created and switched to new branch: %s\n", newBranch)

			// Step 2: Apply the file changes.
			if len(in.Changes) == 0 {
				return ErrorResponse("No changes provided to apply."), nil
			}
			repoRoot := realpath(in.LocalPath)
			paths := make([]string, 0, len(in.Changes))
			for p := range in.Changes {
				paths = append(paths, p)
			}
			sort.Strings(paths)
			for _, rel := range paths {
				full := realpath(filepath.Join(in.LocalPath, rel))
				if !(full == repoRoot || strings.HasPrefix(full, repoRoot+string(os.PathSeparator))) {
					return ErrorResponse(fmt.Sprintf("Access denied: change path escapes the repository: %s", rel)), nil
				}
				if err := os.MkdirAll(filepath.Dir(full), 0o755); err != nil {
					return fail(err)
				}
				if err := os.WriteFile(full, []byte(in.Changes[rel]), 0o644); err != nil {
					return fail(err)
				}
				fmt.Printf("Applied changes to %s\n", rel)
			}
			// Step 3: Stage the changes.
			if err := git("add", "."); err != nil {
				return fail(err)
			}
			fmt.Println("Staged all changes.")
			// Step 4: Commit the changes.
			if err := git("commit", "-m", in.CommitMessage); err != nil {
				return fail(err)
			}
			fmt.Printf("Committed changes with message: '%s'\n", in.CommitMessage)
			// Step 5: Push the new branch to the remote repository.
			if err := git("push", "-u", "origin", newBranch); err != nil {
				return fail(err)
			}
			fmt.Printf("Pushed branch '%s' to origin.\n", newBranch)
			// Step 6: Create the pull request via GitHub API.
			url := fmt.Sprintf("%s/repos/%s/%s/pulls", GitHubBaseURL, in.RepoOwner, in.RepoName)
			pr, err := PostRequest(url, map[string]any{
				"title": in.PRTitle, "body": in.PRBody, "head": newBranch, "base": in.BaseBranch,
			})
			if err != nil {
				return fail(err)
			}
			fmt.Printf("Successfully created pull request: %s\n", team.PyStr(team.Get(pr, "html_url")))
			return map[string]any{"status": "success", "pull_request": pr}, nil
		})
}

// IssueArgs are get_issue's arguments.
type IssueArgs struct {
	RepoOwner   string `json:"repo_owner"`
	RepoName    string `json:"repo_name"`
	IssueNumber int    `json:"issue_number"`
}

// GetIssueFunc is get_issue's secured function, for calling it directly
// (Python's main calls get_issue(...) outside the agent).
var GetIssueFunc = team.Secured(func(_ agent.Context, in IssueArgs) (map[string]any, error) {
	url := fmt.Sprintf("%s/repos/%s/%s/issues/%d", GitHubBaseURL, in.RepoOwner, in.RepoName, in.IssueNumber)
	resp, err := GetRequest(url, nil)
	if err != nil {
		return ErrorResponse(fmt.Sprintf("Error: %v", err)), nil
	}
	return map[string]any{"status": "success", "issue": resp}, nil
})

// GetIssue is the get_issue tool.
func GetIssue() tool.Tool {
	return team.Tool("get_issue", `Get the details of the specified issue number.

Args:
    repo_owner: The name of the repository owner.
    repo_name: The name of the repository.
    issue_number: issue number of the GitHub issue.

Returns:
  The status of this request, with the issue details when successful.`, GetIssueFunc)
}

// CreateIssue is the create_issue tool.
func CreateIssue() tool.Tool {
	type args struct {
		RepoOwner string `json:"repo_owner"`
		RepoName  string `json:"repo_name"`
		Title     string `json:"title"`
		Body      string `json:"body"`
	}
	return team.SecuredTool("create_issue", `Create a new issue in the specified repository.

Args:
    repo_owner: The name of the repository owner.
    repo_name: The name of the repository.
    title: The title of the issue.
    body: The body of the issue.

Returns:
    The status of this request, with the issue details when successful.`,
		func(_ agent.Context, in args) (map[string]any, error) {
			url := fmt.Sprintf("%s/repos/%s/%s/issues", GitHubBaseURL, in.RepoOwner, in.RepoName)
			resp, err := PostRequest(url, map[string]any{"title": in.Title, "body": in.Body, "labels": []string{"docs updates"}})
			if err != nil {
				return ErrorResponse(fmt.Sprintf("Error: %v", err)), nil
			}
			return map[string]any{"status": "success", "issue": resp}, nil
		})
}

// UpdateIssue is the update_issue tool.
func UpdateIssue() tool.Tool {
	type args struct {
		RepoOwner   string `json:"repo_owner"`
		RepoName    string `json:"repo_name"`
		IssueNumber int    `json:"issue_number"`
		Title       string `json:"title"`
		Body        string `json:"body"`
	}
	return team.SecuredTool("update_issue", `Update an existing issue in the specified repository.

Args:
    repo_owner: The name of the repository owner.
    repo_name: The name of the repository.
    issue_number: The number of the issue to update.
    title: The title of the issue.
    body: The body of the issue.

Returns:
    The status of this request, with the issue details when successful.`,
		func(_ agent.Context, in args) (map[string]any, error) {
			url := fmt.Sprintf("%s/repos/%s/%s/issues/%d", GitHubBaseURL, in.RepoOwner, in.RepoName, in.IssueNumber)
			resp, err := PatchRequest(url, map[string]any{"title": in.Title, "body": in.Body})
			if err != nil {
				return ErrorResponse(fmt.Sprintf("Error: %v", err)), nil
			}
			return map[string]any{"status": "success", "issue": resp}, nil
		})
}

// GetFileDiffForRelease is the get_file_diff_for_release tool.
func GetFileDiffForRelease() tool.Tool {
	type args struct {
		RepoOwner string `json:"repo_owner"`
		RepoName  string `json:"repo_name"`
		StartTag  string `json:"start_tag"`
		EndTag    string `json:"end_tag"`
		FilePath  string `json:"file_path"`
	}
	return team.SecuredTool("get_file_diff_for_release", `Gets the diff/patch for a specific file between two release tags.

This is useful for incremental processing where you want to analyze
one file at a time instead of loading all changes at once.

Args:
    repo_owner: The name of the repository owner.
    repo_name: The name of the repository.
    start_tag: The older tag (base) for the comparison.
    end_tag: The newer tag (head) for the comparison.
    file_path: The relative path of the file to get the diff for.

Returns:
    A dictionary containing the status and the file diff details.`,
		func(_ agent.Context, in args) (map[string]any, error) {
			data, err := GetRequest(compareURL(in.RepoOwner, in.RepoName, in.StartTag, in.EndTag), nil)
			if err != nil {
				return requestError(err), nil
			}
			for _, f := range team.List(getOr(data, "files", []any{})) {
				if fn, ok := team.Get(f, "filename").(string); ok && fn == in.FilePath {
					return map[string]any{"status": "success", "file": fileEntry(f, true)}, nil
				}
			}
			return ErrorResponse(fmt.Sprintf("File %s not found in the comparison.", in.FilePath)), nil
		})
}

// GetChangedFilesSummary is the get_changed_files_summary tool.
func GetChangedFilesSummary() tool.Tool {
	type args struct {
		RepoOwner     string `json:"repo_owner"`
		RepoName      string `json:"repo_name"`
		StartTag      string `json:"start_tag"`
		EndTag        string `json:"end_tag"`
		LocalRepoPath string `json:"local_repo_path,omitempty"`
		PathFilter    string `json:"path_filter,omitempty"`
	}
	return team.SecuredTool("get_changed_files_summary", `Gets a summary of changed files between two releases without patches.

This function uses local git commands when local_repo_path is provided,
which avoids the GitHub API's 300-file limit for large comparisons.
Falls back to GitHub API if local_repo_path is not provided or invalid.

Args:
    repo_owner: The name of the repository owner.
    repo_name: The name of the repository.
    start_tag: The older tag (base) for the comparison.
    end_tag: The newer tag (head) for the comparison.
    local_repo_path: Optional absolute path to local git repo. If provided
        and valid, uses git diff instead of GitHub API to get complete
        file list (avoids 300-file limit).
    path_filter: Optional path prefix to filter files. Only files whose
        path starts with this prefix will be included. Example:
        "src/google/adk/" to only include ADK source files.

Returns:
    A dictionary containing the status and a summary of changed files.`,
		func(_ agent.Context, in args) (map[string]any, error) {
			if in.LocalRepoPath != "" {
				if st, err := os.Stat(filepath.Join(in.LocalRepoPath, ".git")); err == nil && st.IsDir() {
					return changedFilesFromLocalGit(in.LocalRepoPath, in.StartTag, in.EndTag, in.RepoOwner, in.RepoName, in.PathFilter), nil
				}
			}
			data, err := GetRequest(compareURL(in.RepoOwner, in.RepoName, in.StartTag, in.EndTag), nil)
			if err != nil {
				return requestError(err), nil
			}
			byDir := map[string]any{}
			files := []any{}
			for _, f := range team.List(getOr(data, "files", []any{})) {
				info := fileEntry(f, false)
				files = append(files, info)
				path := team.Str(getOr(f, "filename", ""))
				top := strings.Split(path, "/")[0]
				byDir[top] = append(team.List(byDir[top]), info)
			}
			var note any
			if len(files) >= 300 {
				note = "Using GitHub API which is limited to 300 files. Provide local_repo_path to get complete file list."
			}
			return map[string]any{
				"status":             "success",
				"total_files":        len(files),
				"files":              files,
				"files_by_directory": byDir,
				"compare_url":        fmt.Sprintf("https://github.com/%s/%s/compare/%s...%s", in.RepoOwner, in.RepoName, in.StartTag, in.EndTag),
				"note":               note,
			}, nil
		})
}

// changedFilesFromLocalGit is _get_changed_files_from_local_git.
func changedFilesFromLocalGit(repoPath, startTag, endTag, owner, repo, pathFilter string) map[string]any {
	fail := func(err error) map[string]any {
		var ge *gitError
		if errors.As(err, &ge) {
			return ErrorResponse(fmt.Sprintf("Git command failed: %s", ge.stderr))
		}
		return ErrorResponse(fmt.Sprintf("Error getting changed files: %s", osErr(err)))
	}
	// Fetch tags to ensure we have them.
	if _, _, _, err := runGit(repoPath, false, "fetch", "--tags"); err != nil {
		return fail(err)
	}
	diff, _, _, err := runGit(repoPath, true, "diff", "--name-status", startTag+"..."+endTag)
	if err != nil {
		return fail(err)
	}
	numstat, _, _, err := runGit(repoPath, true, "diff", "--numstat", startTag+"..."+endTag)
	if err != nil {
		return fail(err)
	}

	type stats struct{ additions, deletions, changes int }
	fileStats := map[string]stats{}
	for _, line := range strings.Split(strings.TrimSpace(numstat), "\n") {
		if line == "" {
			continue
		}
		parts := strings.Split(line, "\t")
		if len(parts) >= 3 {
			num := func(s string) (int, error) {
				if s == "-" {
					return 0, nil
				}
				n, err := strconv.Atoi(s)
				if err != nil {
					return 0, fmt.Errorf("invalid literal for int() with base 10: '%s'", s)
				}
				return n, nil
			}
			a, err := num(parts[0])
			if err != nil {
				return fail(err)
			}
			d, err := num(parts[1])
			if err != nil {
				return fail(err)
			}
			fileStats[parts[2]] = stats{a, d, a + d}
		}
	}

	statusMap := map[byte]string{'A': "added", 'D': "removed", 'M': "modified", 'R': "renamed", 'C': "copied"}
	byDir := map[string]any{}
	files := []any{}
	for _, line := range strings.Split(strings.TrimSpace(diff), "\n") {
		if line == "" {
			continue
		}
		parts := strings.Split(line, "\t")
		if len(parts) < 2 {
			continue
		}
		filename := parts[len(parts)-1]
		if pathFilter != "" && !strings.HasPrefix(filename, pathFilter) {
			continue
		}
		st := fileStats[filename]
		status, ok := statusMap[parts[0][0]]
		if !ok {
			status = "modified"
		}
		info := map[string]any{
			"relative_path": filename,
			"status":        status,
			"additions":     st.additions,
			"deletions":     st.deletions,
			"changes":       st.changes,
		}
		files = append(files, info)
		top := strings.Split(filename, "/")[0]
		byDir[top] = append(team.List(byDir[top]), info)
	}
	return map[string]any{
		"status":             "success",
		"total_files":        len(files),
		"files":              files,
		"files_by_directory": byDir,
		"compare_url":        fmt.Sprintf("https://github.com/%s/%s/compare/%s...%s", owner, repo, startTag, endTag),
	}
}
