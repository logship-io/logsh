#pragma once

#define LOG_DEBUG(MSG, ...) logsh::shared::LogDebug(MSG, ...)

namespace logsh::shared {
	template <typename ...TArgs>
	void LogDebug(const char* fmt, TArgs... args);
};