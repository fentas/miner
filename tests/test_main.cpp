// BTC Miner Test Suite - Main Runner

#include "test_framework.hpp"

// Test modules
namespace test_sha256 { void run_all(); }
namespace test_sha256_avx { void run_all(); }
namespace test_stratum { void run_all(); }
namespace test_stratum_v2 { void run_all(); }
namespace test_gpu { void run_all(); }
namespace test_config { void run_all(); }
namespace test_connection { void run_all(); }

int main(int argc, char* argv[]) {
    std::cout << "╔══════════════════════════════════════════════════════════╗\n"
              << "║              BTC Miner Test Suite                        ║\n"
              << "╚══════════════════════════════════════════════════════════╝\n";

    // Parse arguments for selective testing
    bool run_sha256 = true;
    bool run_avx = true;
    bool run_stratum = true;
    bool run_stratum_v2 = true;
    bool run_gpu = true;
    bool run_config = true;
    bool run_connection = true;

    for (int i = 1; i < argc; i++) {
        std::string arg = argv[i];
        if (arg == "--sha256") {
            run_sha256 = true;
            run_avx = run_stratum = run_stratum_v2 = run_gpu = run_config = run_connection = false;
        } else if (arg == "--avx") {
            run_avx = true;
            run_sha256 = run_stratum = run_stratum_v2 = run_gpu = run_config = run_connection = false;
        } else if (arg == "--stratum") {
            run_stratum = true;
            run_sha256 = run_avx = run_stratum_v2 = run_gpu = run_config = run_connection = false;
        } else if (arg == "--stratum-v2") {
            run_stratum_v2 = true;
            run_sha256 = run_avx = run_stratum = run_gpu = run_config = run_connection = false;
        } else if (arg == "--gpu") {
            run_gpu = true;
            run_sha256 = run_avx = run_stratum = run_stratum_v2 = run_config = run_connection = false;
        } else if (arg == "--config") {
            run_config = true;
            run_sha256 = run_avx = run_stratum = run_stratum_v2 = run_gpu = run_connection = false;
        } else if (arg == "--connection") {
            run_connection = true;
            run_sha256 = run_avx = run_stratum = run_stratum_v2 = run_gpu = run_config = false;
        } else if (arg == "--help" || arg == "-h") {
            std::cout << "\nUsage: " << argv[0] << " [OPTIONS]\n\n"
                      << "Options:\n"
                      << "  --sha256      Run only SHA-256 tests\n"
                      << "  --avx         Run only AVX2/AVX512 tests\n"
                      << "  --stratum     Run only Stratum V1 tests\n"
                      << "  --stratum-v2  Run only Stratum V2 tests\n"
                      << "  --gpu         Run only GPU tests\n"
                      << "  --config      Run only config validation tests\n"
                      << "  --connection  Run only pool connection tests\n"
                      << "  --help        Show this help\n"
                      << "\nWithout options, all tests are run.\n";
            return 0;
        }
    }

    // Run selected tests
    if (run_sha256) {
        test_sha256::run_all();
    }

    if (run_avx) {
        test_sha256_avx::run_all();
    }

    if (run_stratum) {
        test_stratum::run_all();
    }

    if (run_stratum_v2) {
        test_stratum_v2::run_all();
    }

    if (run_gpu) {
        test_gpu::run_all();
    }

    if (run_config) {
        test_config::run_all();
    }

    if (run_connection) {
        test_connection::run_all();
    }

    // Print summary
    test::TestSuite::instance().summary();

    return test::TestSuite::instance().failed() > 0 ? 1 : 0;
}
