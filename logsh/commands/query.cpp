#include "query.h"

namespace logsh::commands {
void Query::CreateCommand(CLI::App &parent)
{
    const auto queryCommand = parent.add_subcommand("query", "Query the ship.");

}
};