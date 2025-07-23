#!/bin/bash

# ABOUTME: Comprehensive test runner script for local development and CI
# ABOUTME: Runs all test types with proper reporting and error handling

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
VERBOSE=${VERBOSE:-false}
COVERAGE=${COVERAGE:-false}
BENCHMARKS=${BENCHMARKS:-false}
CLIPPY_PEDANTIC=${CLIPPY_PEDANTIC:-false}

print_header() {
    echo -e "${BLUE}===================================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}===================================================${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

run_command() {
    local cmd="$1"
    local description="$2"

    echo -e "${BLUE}Running:${NC} $description"
    if [ "$VERBOSE" = true ]; then
        echo -e "${YELLOW}Command:${NC} $cmd"
    fi

    if eval "$cmd"; then
        print_success "$description completed"
        return 0
    else
        print_error "$description failed"
        return 1
    fi
}

check_prerequisites() {
    print_header "Checking Prerequisites"

    # Check Rust installation
    if command -v cargo &> /dev/null; then
        local rust_version=$(rustc --version)
        print_success "Rust found: $rust_version"
    else
        print_error "Rust not found. Please install Rust from https://rustup.rs/"
        exit 1
    fi

    # Check for required components
    if rustup component list --installed | grep -q "rustfmt"; then
        print_success "rustfmt component found"
    else
        print_warning "rustfmt not found, installing..."
        rustup component add rustfmt
    fi

    if rustup component list --installed | grep -q "clippy"; then
        print_success "clippy component found"
    else
        print_warning "clippy not found, installing..."
        rustup component add clippy
    fi
}

run_formatting_check() {
    print_header "Code Formatting Check"
    run_command "cargo fmt --all -- --check" "Code formatting check"
}

run_clippy() {
    print_header "Clippy Linting"

    local clippy_cmd="cargo clippy --all-targets --all-features"

    if [ "$CLIPPY_PEDANTIC" = true ]; then
        clippy_cmd="$clippy_cmd -- -D warnings -D clippy::pedantic"
        run_command "$clippy_cmd" "Clippy (pedantic mode)"
    else
        clippy_cmd="$clippy_cmd -- -D warnings"
        run_command "$clippy_cmd" "Clippy (standard mode)"
    fi
}

run_build() {
    print_header "Build Check"
    run_command "cargo build --all-features" "Debug build"
    run_command "cargo build --release --all-features" "Release build"
}

run_unit_tests() {
    print_header "Unit Tests"

    local test_cmd="cargo test --lib"
    if [ "$VERBOSE" = true ]; then
        test_cmd="$test_cmd -- --nocapture"
    fi

    run_command "$test_cmd" "Unit tests"
}

run_integration_tests() {
    print_header "Integration Tests"

    local test_files=(
        "integration_tests"
        "property_tests"
        "error_handling_tests"
    )

    for test_file in "${test_files[@]}"; do
        local test_cmd="cargo test --test $test_file"
        if [ "$VERBOSE" = true ]; then
            test_cmd="$test_cmd -- --nocapture"
        fi

        run_command "$test_cmd" "Integration test: $test_file"
    done
}

run_doc_tests() {
    print_header "Documentation Tests"
    run_command "cargo test --doc" "Documentation tests"
}

run_all_tests() {
    print_header "All Tests"

    local test_cmd="cargo test --all-targets --all-features"
    if [ "$VERBOSE" = true ]; then
        test_cmd="$test_cmd -- --nocapture"
    fi

    run_command "$test_cmd" "All tests"
}

run_coverage() {
    if [ "$COVERAGE" != true ]; then
        return 0
    fi

    print_header "Code Coverage"

    # Check if cargo-llvm-cov is installed
    if ! cargo llvm-cov --version &> /dev/null; then
        print_warning "cargo-llvm-cov not found, installing..."
        cargo install cargo-llvm-cov
    fi

    run_command "cargo llvm-cov --all-features --workspace --lcov --output-path target/lcov.info" "Generate coverage report"
    run_command "cargo llvm-cov --all-features --workspace --html" "Generate HTML coverage report"

    print_success "Coverage report generated in target/llvm-cov/html/index.html"
}

run_benchmarks() {
    if [ "$BENCHMARKS" != true ]; then
        return 0
    fi

    print_header "Performance Benchmarks"

    run_command "cargo bench --bench log_parsing" "Log parsing benchmarks"
    run_command "cargo bench --bench api_performance" "API performance benchmarks"

    print_success "Benchmark results available in target/criterion/"
}

run_security_audit() {
    print_header "Security Audit"

    # Check if cargo-audit is installed
    if ! cargo audit --version &> /dev/null; then
        print_warning "cargo-audit not found, installing..."
        cargo install cargo-audit
    fi

    run_command "cargo audit" "Security audit"

    # Check for unsafe code
    if grep -r "unsafe" src/ &> /dev/null; then
        print_warning "Unsafe code found in src/ directory"
        grep -rn "unsafe" src/ || true
    else
        print_success "No unsafe code found"
    fi
}

run_doc_generation() {
    print_header "Documentation Generation"
    run_command "cargo doc --no-deps --all-features" "Generate documentation"
}

generate_test_report() {
    print_header "Test Summary"

    local total_tests=$(cargo test --all-targets --all-features 2>&1 | grep -E "test result:" | tail -1 | sed -n 's/.*test result: [^0-9]*\([0-9]\+\).*/\1/p')

    if [ -n "$total_tests" ]; then
        print_success "Total tests run: $total_tests"
    fi

    if [ -f "target/lcov.info" ]; then
        local coverage_percent=$(grep -E "LF:|LH:" target/lcov.info | awk -F: '{lines+=$2; hit+=$2*($1=="LH")} END {printf "%.1f", hit/lines*100}')
        print_success "Code coverage: ${coverage_percent}%"
    fi

    echo ""
    print_success "All tests completed successfully!"
    echo ""
    echo "Generated reports:"
    [ -d "target/llvm-cov/html" ] && echo "  - Coverage: target/llvm-cov/html/index.html"
    [ -d "target/criterion" ] && echo "  - Benchmarks: target/criterion/report/index.html"
    [ -d "target/doc" ] && echo "  - Documentation: target/doc/cc_log_viewer/index.html"
}

cleanup() {
    print_header "Cleanup"
    # Kill any background processes if needed
    jobs -p | xargs -r kill 2>/dev/null || true
}

show_help() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -v, --verbose           Enable verbose output"
    echo "  -c, --coverage         Generate code coverage report"
    echo "  -b, --benchmarks       Run performance benchmarks"
    echo "  -p, --pedantic         Use pedantic clippy lints"
    echo "  --unit-only            Run only unit tests"
    echo "  --integration-only     Run only integration tests"
    echo "  --quick                Skip slow tests (no benchmarks/coverage)"
    echo "  -h, --help             Show this help message"
    echo ""
    echo "Environment variables:"
    echo "  VERBOSE=true           Same as --verbose"
    echo "  COVERAGE=true          Same as --coverage"
    echo "  BENCHMARKS=true        Same as --benchmarks"
    echo "  CLIPPY_PEDANTIC=true   Same as --pedantic"
}

main() {
    local unit_only=false
    local integration_only=false
    local quick_mode=false

    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -v|--verbose)
                VERBOSE=true
                shift
                ;;
            -c|--coverage)
                COVERAGE=true
                shift
                ;;
            -b|--benchmarks)
                BENCHMARKS=true
                shift
                ;;
            -p|--pedantic)
                CLIPPY_PEDANTIC=true
                shift
                ;;
            --unit-only)
                unit_only=true
                shift
                ;;
            --integration-only)
                integration_only=true
                shift
                ;;
            --quick)
                quick_mode=true
                COVERAGE=false
                BENCHMARKS=false
                shift
                ;;
            -h|--help)
                show_help
                exit 0
                ;;
            *)
                print_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done

    # Set up error handling
    trap cleanup EXIT

    echo -e "${GREEN}Starting cc-log-viewer test suite...${NC}"
    echo ""

    # Run checks and tests
    check_prerequisites

    if [ "$unit_only" = false ] && [ "$integration_only" = false ]; then
        run_formatting_check
        run_clippy
        run_build
    fi

    if [ "$integration_only" = false ]; then
        run_unit_tests
    fi

    if [ "$unit_only" = false ]; then
        run_integration_tests
        run_doc_tests
    fi

    if [ "$unit_only" = false ] && [ "$integration_only" = false ]; then
        run_security_audit
        run_doc_generation
    fi

    if [ "$quick_mode" = false ]; then
        run_coverage
        run_benchmarks
    fi

    generate_test_report
}

# Run main function
main "$@"
