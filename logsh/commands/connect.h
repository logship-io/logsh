#pragma once
#include "Cli11.hpp"


namespace logsh::commands {
	class Connect {
		public:
			static void CreateCommand(CLI::App& parent);
	};
};