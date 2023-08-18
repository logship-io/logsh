#include "connect.h"

#include <iostream>
#include "../shared/configuration.h"

namespace logsh::commands {
	void Connect::CreateCommand(CLI::App& parent)
	{
		auto connectCommand = parent.add_subcommand("connect", "Connect to a logship server.");
		auto server = connectCommand->add_option("server", "Server endpoint.")->required();

		connectCommand->callback([&]() {
			auto config = shared::ConfigurationInstance();
			std::cout << "YOOOOOO" << std::endl;
			config.Save();
			});
	}
}