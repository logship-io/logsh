// logsh.cpp : Defines the entry point for the application.
//

#include "logsh.h"
#include "CLI11.hpp"
#include "shared/logger.h"
#include "commands/connect.h"

int main(int argc, char* argv[])
{
	CLI::App app{"Logship Command Line"};
	logsh::commands::Connect::CreateCommand(app);

	CLI11_PARSE(app, argc, argv);
	return 0;
}
