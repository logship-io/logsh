#pragma once

#include "Cli11.hpp"

namespace logsh::commands {
    class Query {
        public:
            static void CreateCommand(CLI::App& parent);
    };
};