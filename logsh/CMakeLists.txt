# CMakeList.txt : CMake project for logsh, include source and define
# project specific logic here.
#
set(Boost_USE_STATIC_LIBS ON)
find_package(Boost REQUIRED COMPONENTS json)

# Add source to this project's executable.
add_executable (logsh 
	"logsh.cpp"
	"logsh.h"
	"commands/query.cpp"
	"commands/query.h"
	"shared/configuration.cpp"
	"shared/configuration.h"
	"shared/logger.h"
	"shared/logger.cpp" "commands/connect.h" "commands/connect.cpp" )

target_link_libraries(logsh PRIVATE ${Boost_LIBRARIES})
#target_include_directories(logsh PRIVATE ${Boost_INCLUDE_DIRS})
target_include_directories (logsh PRIVATE include)
include_directories(${Boost_INCLUDE_DIRS})

if (CMAKE_VERSION VERSION_GREATER 3.12)
  set_property(TARGET logsh PROPERTY CXX_STANDARD 20)
endif()

# TODO: Add tests and install targets if needed.
