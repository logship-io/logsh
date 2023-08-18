#include "configuration.h"
#include <iostream>
#include <iterator>
#include <fstream>
#include <string>
#include <filesystem>
#include <boost/json.hpp>

namespace logsh::shared {
	std::optional<Configuration> cached_config = std::nullopt;
	static const std::string USER_CONFIG_PATH = ".logsh.json";

	std::filesystem::path getHomeDir() {
		auto homeDir = std::getenv("HOME");
		if (!homeDir) {
			homeDir = std::getenv("HOMEPATH");
		}
		return std::string(homeDir);
	}

	std::optional<Configuration> loadConfig()
	{
		auto file = getHomeDir() / USER_CONFIG_PATH;
		if (!std::filesystem::exists(file)) {
			return std::nullopt;
		}

		std::ifstream configFile(file.string());
		if (!configFile)
		{
			return std::nullopt;
		}
		std::string fileAsString;
		fileAsString.assign(std::istreambuf_iterator<char>(configFile), std::istreambuf_iterator<char>());

		std::error_code err;
		auto json = boost::json::parse(fileAsString, err);

		Configuration result{};
		result = boost::json::value_to<Configuration>(json);

		return result;
	}

	Configuration ConfigurationInstance()
	{
		if (!cached_config) {
			cached_config = loadConfig();
			if (!cached_config) {
				cached_config = Configuration{};
			}
		}

		return cached_config.value();
	}

	void Configuration::Save()
	{
		auto file = getHomeDir() / USER_CONFIG_PATH;
		if (!std::filesystem::exists(file)) {
			std::filesystem::create_directories(file.parent_path());
		}

		std::ofstream configFile(file.string());
		if (!configFile)
		{
			std::cout << "Cannot write file to disk!?" << std::endl;
			return;
		}

		auto json = boost::json::value_from(*this);
		configFile << boost::json::serialize(json);

		std::cout << "Writing json: " << boost::json::serialize(json) << " to file: " << file << std::endl;


		configFile.flush();
		configFile.close();
	}
}


#define PARSER_FUNC(type)								\
type tag_invoke(boost::json::value_to_tag<type>, boost::json::value const& value)

#define WRITER_FUNC(type)								\
void tag_invoke(const boost::json::value_from_tag&, boost::json::value& value, type const& addr)

#define JSON_FIELD(object, json_container, field)       \
do {                                                    \
    if (!json_container.contains(#field)) {             \
        break;                                          \
    }                                                   \
                                                        \
    parse(object.field, json_container.at(#field));     \
} while (false);


namespace boost::json {
	template<typename T> inline void parse(T& strField, const boost::json::value& value) {
		strField = boost::json::value_to<T>(value);
	};

	PARSER_FUNC(logsh::shared::Configuration)
	{
		auto& obj = value.as_object();
		logsh::shared::Configuration output;
		JSON_FIELD(output, obj, connections);
		return (logsh::shared::Configuration)output;
	};

	WRITER_FUNC(logsh::shared::Configuration)
	{
		value = boost::json::object{
			//{"connections", addr.connections}
		};
	};

	PARSER_FUNC(logsh::shared::ConnectionInfo)
	{
		auto& obj = value.as_object();
		logsh::shared::ConnectionInfo output;
		//JSON_FIELD(output, obj, endpoint);
		return (logsh::shared::ConnectionInfo)output;
	};

	WRITER_FUNC(logsh::shared::ConnectionInfo)
	{
		value = boost::json::object{
			{"endpoint", addr.endpoint}
		};
	};
};
