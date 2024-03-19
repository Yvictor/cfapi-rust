#ifndef CFAPI_SESSIONCONFIG_H
#define CFAPI_SESSIONCONFIG_H 1

#include <string>

#include "dllexport.h"

namespace cfapi_internal
{
	class SessionConfigImpl;
	class SessionImpl;
}

namespace cfapi
{

/**
 * The SessionConfig is the configuration options for the entire Session
 */

class SessionConfig
{
	public:
		/**
		* Session Configuration Parameters
		*/
		enum Parameters
		{
			/**
			* Deprecated - do not use
			*/
			RECOVERY_BOOL=0,

			/**
			*Indicate whether API should create mulitple threads to handle multiple CSP connections.  Default is false.
			*/
			MULTITHREADED_API_CONNECTIONS_BOOL=1,

			/**
			*When MULTITHREADED_API_CONNECTIONS_BOOL=true, this indicates the maximum number of user-side threads the API should create.  If set to 0 (default), <0, or not set, the API will open as many threads as it can use.
			*/
			MAX_USER_THREADS_LONG=2,

			/**
			*When MULTITHREADED_API_CONNECTIONS_BOOL=true, this indicates the maximum number of CSP-side (backend) threads the API should create.  If set to 0 (default), <0, or not set, the API will open as many threads as it can use.
			*/
			MAX_CSP_THREADS_LONG=3,

			/**
			*Maximum number of requests to be queued before sending to the CSP.  Default is 100000 (without watchlist) or 10000000 (with watchlist); valid range is 100000-50000000 (requests)
			*When the queue is full, any further send() requests will fail.
			*/
			MAX_REQUEST_QUEUE_SIZE_LONG=4,

			/**
			*Indicate whether API should use watchlist to manage requests.  Default is false.
			*/
			WATCHLIST_BOOL=5,

			/**
			*Maximum number of requests to be added to watchlist.  Default is 10000000 (requests)
			*/
			MAX_WATCHLIST_SIZE_LONG=6,

			/**
			*Threshold to trigger CFAPI_SESSION_RECEIVE_QUEUE_ABOVE_THRESHOLD and CFAPI_SESSION_RECEIVE_QUEUE_BELOW_THRESHOLD SessionEvents.  Default is 70%; valid range is 1-101.
			*/
			QUEUE_DEPTH_THRESHOLD_PERCENT_LONG=7,

		};
		
		/**
		* Sets session parameters. If not previously set, this will create and set value. If already set, this will replace setting
		* 
		* @param parameter
		*            A SessionConfig.Parameter value that corresponds to the config value
		* @param value
		*            The value of the desired parameter
		* @return this pointer for chaining
		*/
		CFAPI_DLLEXPORTS SessionConfig& set(Parameters parameter, long value);

		/**
		* Sets session parameters. If not previously set, this will create and set value. If already set, this will replace setting
		* 
		* @param parameter
		*            A SessionConfig.Parameter value that corresponds to the config value
		* @param value
		*            The value of the desired parameter
		* @return this pointer for chaining
		*/
		CFAPI_DLLEXPORTS SessionConfig& set(Parameters parameter, bool value);

	private:
		SessionConfig();
		~SessionConfig();
		cfapi_internal::SessionConfigImpl *sessionConfigImpl;
		friend class cfapi_internal::SessionImpl;
};
}

#endif //CFAPI_SESSIONCONFIG_H
