#ifndef CFAPI_HOSTCONFIG_H
#define CFAPI_HOSTCONFIG_H 1

#include <string>

#include "dllexport.h"

namespace cfapi_internal
{
	class HostConfigImpl;
	class CspClient;
	class SessionImpl;
	class ConnectionConfigImpl;
}


namespace cfapi
{

class ConnectionConfig;

/**
 * The HostConfig is the configuration options for a connection to specific CSP.
 * These settings will override the global settings in ConnectionConfig if set
 */
class  HostConfig
{
	public:

		/**
		* Connection/Host Configuration Parameters
		*/
		enum Parameters
		{
			/**
			*Is this a backup CSP?  Default is false.
			*/
			BACKUP_BOOL=0,
			/**
			*Use compression between CSP and API.  Default is true
			*/
			COMPRESSION_BOOL=1,	
			/**
			*Indicate which messages are conflatable.  Default is false
			*/
			CONFLATION_INDICATOR_BOOL=2,
			/**
			*Maximum time to hold conflatable data (in milliseconds).  Default is 1000 (ms)
			*Deprecated -  but added new command SETCONFLATIONINTERVAL to set the time
			*/
			CONFLATION_INTERVAL_LONG=3,
			/**
			*Maximum time to wait for a heartbeat message from the CSP.  Default is 5 (seconds)
			*Note: during initialization, this is the maximum time to wait for any response from the CSP.
			*/
			READ_TIMEOUT_LONG=4, 
			/**
			*Maximum time to wait to establish a TCP connection to the CSP.  Default is 5 (seconds)
			*/
			CONNECTION_TIMEOUT_LONG=5,
			/**
			*Maximum number of consecutive reconnects to the CSP without successfully initializing.  Default is 5
			*/
			CONNECTION_RETRY_LIMIT_LONG=6,
			/**
			*Size in megabytes of internal buffer for incoming messages from the CSP.  Default is 1 (megabyte); valid range is 1-256 (MB)
			*Note: there is one queue per backend CSP, so each one will be allocated with this size
			*/
			QUEUE_SIZE_LONG=7,
			/**
  			*Maximum time socket select should block, waiting for a connection to be ready; Default is 200 millisec
  			*/
			BLOCKING_CONNECTION_TIME_LIMIT_LONG=8,

			/**
			*Type of conflation.  1: trade-safe (Default); 2: intervalized; 3: Just-In-Time (JIT)
			*/
			CONFLATION_TYPE_LONG=9,

			/**
			*Threshold to trigger JIT conflation.  This is the percent of the buffer used before feed starts conflating. Default is 25%; valid range is 1-75.  Only valid when CONFLATION_TYPE_LONG is set to 3 (JIT).
			*/
			JIT_CONFLATION_THRESHOLD_PERCENT_LONG=10,
		};

		/**
		* Sets connection parameters. If not previously set, this will create and set value. If already set, this will replace setting
		* Note: Should call before session is started; if called after session start, effect is undefined.
		* 
		* @param parameter
		*            A Parameters value that corresponds to the config value
		* @param value
		*            The value of the desired parameter
		* @return this pointer for chaining
		*/
		CFAPI_DLLEXPORTS HostConfig& set(Parameters parameter, bool value);

		/**
		* Sets connection parameters. If not previously set, this will create and set value. If already set, this will replace setting
		* Note: Should call before session is started; if called after session start, effect is undefined.
		* 
		* @param parameter
		*            A Parameters value that corresponds to the config value
		* @param value
		*            The value of the desired parameter
		* @return this pointer for chaining
		*/
		CFAPI_DLLEXPORTS HostConfig& set(Parameters parameter, long value);

	private:
		HostConfig();
		HostConfig(std::string ipAddr, size_t port, ConnectionConfig *connectionConfig, size_t index);
		~HostConfig();
		cfapi_internal::HostConfigImpl *hostCfgImpl;
		friend class cfapi_internal::CspClient;
		friend class cfapi_internal::SessionImpl;
		friend class cfapi_internal::ConnectionConfigImpl;
};
}

#endif // CFAPI_HOSTCONFIG_H
