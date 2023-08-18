#pragma once
#include <optional>
#include <string>
#include <vector>

namespace logsh::shared {

	class ConnectionInfo {
	public:
		std::string endpoint;
	};

	class Configuration {
		public:
			void Save();

			std::vector<ConnectionInfo> connections;
	};

	Configuration ConfigurationInstance();
};
