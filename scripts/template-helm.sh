#!/usr/bin/env bash

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS] [DIRECTORY...]

Template Helm charts using helmfile and update kustomization.yaml resources.

Arguments:
  DIRECTORY...    One or more directories to search for helmfile.yaml files.
                  If not specified, searches the entire repository.

Options:
  -h, --help      Show this help message and exit
  -v, --verbose   Enable verbose output
  -d, --dry-run   Show what would be done without making changes

Examples:
  $(basename "$0")                                         # Search entire repo
  $(basename "$0") infra/clusters/networking               # Search networking cluster only
  $(basename "$0") infra/clusters/networking/infra/cilium  # Search specific app
  $(basename "$0") infra/clusters/networking infra/clusters/prod  # Multiple directories

EOF
}

# Default values
VERBOSE=false
DRY_RUN=false
SEARCH_DIRS=()

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            exit 0
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -d|--dry-run)
            DRY_RUN=true
            shift
            ;;
        -*)
            print_error "Unknown option: $1"
            usage
            exit 1
            ;;
        *)
            SEARCH_DIRS+=("$1")
            shift
            ;;
    esac
done

check_dependencies() {
    local missing_deps=()

    if ! command -v helmfile &> /dev/null; then
        missing_deps+=("helmfile")
    fi

    if ! command -v kustomize &> /dev/null; then
        missing_deps+=("kustomize")
    fi

    if [ ${#missing_deps[@]} -ne 0 ]; then
        print_error "Missing required dependencies: ${missing_deps[*]}"
        print_error "Please install the missing tools and try again."
        exit 1
    fi
}

find_helmfiles() {
    local search_paths=("$@")
    local helmfiles=()

    for search_path in "${search_paths[@]}"; do
        if [[ ! -d "$search_path" ]]; then
            print_warning "Directory does not exist: $search_path"
            continue
        fi

        while IFS= read -r -d '' file; do
            helmfiles+=("$file")
        done < <(find "$search_path" -name "helmfile.yaml" -type f -print0 2>/dev/null)
    done

    printf '%s\n' "${helmfiles[@]}"
}

process_helmfile() {
    local helmfile="$1"
    local helmfile_dir
    helmfile_dir="$(dirname "$helmfile")"

    print_status "Processing helmfile in: $helmfile_dir"

    if [[ "$DRY_RUN" == "true" ]]; then
        print_status "[DRY RUN] Would process: $helmfile_dir"
        return 0
    fi

    local original_dir
    original_dir="$(pwd)"

    cd "$helmfile_dir"

    # Remove existing helm resources from kustomization
    kustomize edit remove resource 'helm/**/*.yaml' 2>/dev/null || true

    # Remove existing helm directory
    rm -rf helm

    # Template the helmfile
    local stderr_file
    stderr_file=$(mktemp)
    trap "rm -f '$stderr_file'" RETURN

    if [[ "$VERBOSE" == "true" ]]; then
        helmfile template --output-dir-template "$(pwd)/helm"
    else
        if ! helmfile template --output-dir-template "$(pwd)/helm" >/dev/null 2>"$stderr_file"; then
            print_error "helmfile template failed:"
            cat "$stderr_file" >&2
            return 1
        fi
    fi

    # Add new helm resources to kustomization
    if [[ -d helm ]]; then
        while IFS= read -r -d '' yaml_file; do
            local relative_path="${yaml_file#./}"
            if [[ "$VERBOSE" == "true" ]]; then
                kustomize edit add resource "$relative_path"
            else
                kustomize edit add resource "$relative_path" 2>/dev/null || true
            fi
        done < <(find helm -name "*.yaml" -type f -print0 2>/dev/null)
    fi

    cd "$original_dir"
    print_status "Completed processing $helmfile_dir"
}

main() {
    print_status "Starting helmfile rendering process..."
    check_dependencies

    # Determine search directories
    if [[ ${#SEARCH_DIRS[@]} -eq 0 ]]; then
        SEARCH_DIRS=(".")
        print_status "Searching entire repository for helmfile.yaml files..."
    else
        print_status "Searching specified directories: ${SEARCH_DIRS[*]}"
    fi

    # Find all helmfiles
    mapfile -t helmfiles < <(find_helmfiles "${SEARCH_DIRS[@]}")

    if [[ ${#helmfiles[@]} -eq 0 ]]; then
        print_warning "No helmfile.yaml files found"
        exit 0
    fi

    print_status "Found ${#helmfiles[@]} helmfile(s):"
    for hf in "${helmfiles[@]}"; do
        echo "  - $hf"
    done

    # Process each helmfile
    for helmfile in "${helmfiles[@]}"; do
        process_helmfile "$helmfile"
    done

    print_status "Processing complete!"
    print_status "Formatting..."

    # format templated files for consistency
    if [[ "$VERBOSE" == "true" ]]; then
        nix fmt || print_warning "nix fmt failed or not configured"
    else
        nix fmt >/dev/null 2>&1 || print_warning "nix fmt failed or not configured"
    fi

    print_status "Done!"
}

main
