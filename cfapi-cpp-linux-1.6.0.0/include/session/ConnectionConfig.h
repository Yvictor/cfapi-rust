#ifndef CFAPI_CONNECTIONCONFIG_H
#define CFAPI_CONNECTIONCONFIG_H 1

#include <string>

#include "HostConfig.h"

#include "dllexport.h"

namespace cfapi_internal
{
	class SessionImpl;
	class ConnectionConfigImpl;
}


namespace cfapi
{

class HostConfig;

/**
 * The ConnectionConfig is the global configuration options for the Connections to the backend CSPs
 */
class  ConnectionConfig
{
	public:

		/**
		* Get HostConfig object  Create if it does not already exist.
		* Note: Should create new HostConfig objects before session is started; if created after session start, effect is undefined.
		* 
		* @param hostInfoString
		*	Connection info to host.  Format of "host:port"
		*
		* @return pointer to the HostConfig object
		*/
		 CFAPI_DLLEXPORTS  HostConfig& getHostConfig(std::string hostInfoString);

		/**
		* Sets connection parameters. If not previously set, this will create and set value. If already set, this will replace setting
		* Note: Should call before session is started; if called after session start, effect is undefined.
		* 
		* @param parameter
		*            A HostConfig::Parameters value that corresponds to the config value
		* @param value
		*            The value of the desired parameter
		* @return this pointer for chaining
		*/
		 CFAPI_DLLEXPORTS  ConnectionConfig& set(cfapi::HostConfig::Parameters parameter, bool value);

		/**
		* Sets connection parameters. If not previously set, this will create and set value. If already set, this will replace setting
		* Note: Should call before session is started; if called after session start, effect is undefined.
		* 
		* @param parameter
		*            A HostConfig::Parameters value that corresponds to the config value
		* @param value
		*            The value of the desired parameter
		* @return this pointer for chaining
		*/
		 CFAPI_DLLEXPORTS  ConnectionConfig& set(cfapi::HostConfig::Parameters parameter, long value);

		/**
		* Add NAT pair.  This will allow the API to translate the CSP's advertised external IP address to the local IP address needed  
		* to connect to the CSP from the client's network
		* 
		* @param ipCsp
		*            The CSP's advertised external IP address 
		* @param ipClient
		*            The local IP address that must be used to reach the CSP
		* @return this pointer for chaining
		*/
		 CFAPI_DLLEXPORTS  ConnectionConfig& addNATPair(std::string ipCsp, std::string ipClient);


	private:
		ConnectionConfig();
		ConnectionConfig(cfapi_internal::SessionImpl *);
		~ConnectionConfig();
		
		cfapi_internal::ConnectionConfigImpl *connectionConfigImpl;

		friend class cfapi_internal::SessionImpl;
		friend class cfapi_internal::HostConfigImpl;
};
}

#endif // CFAPI_CONNECTIONCONFIG_H
