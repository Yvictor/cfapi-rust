#ifndef CFAPI_REQUEST_H
#define CFAPI_REQUEST_H

#include <string>
#include "RequestParameters.h"
#include "Commands.h"

#include "dllexport.h"

namespace cfapi_internal
{
	class SessionImpl;
	class RequestImpl;
}


namespace cfapi
{    

/**
 * Request to be sent to CSP(s), to be populated with content identification information. Can be created Session.createRequest and can
 * be sent via the associated Session.
 * 
 * If validation is enabled when initializing the API and the passed in parameter value is 'known', the associated value will be
 * validated - this will confirm that the type is legal for the parameter, its setting is within the acceptable range, etc. If it
 * is an 'unknown' parameter, it will be sent along to the server with no validation to allow for users to send new server side
 * parameters without required API upgrade
 * 
 * 
 * @see Session
 * @see RequestParameters
 * 
 */

    class Request
    {
	public:
		/**
		 * set command for CSP request message.
 		 *
 		 * @param command 
 		 * 	An enumerated value corresponding to the command to send to the CSP with this Request
 		 */
		CFAPI_DLLEXPORTS void setCommand(const cfapi::Commands command);

		/**
 		 * A CSP request message. Should be populated with the desired parameters
 		 *
 		 * @param parameter
 		 * 	An enumerated value corresponding to one of the RequestParameters known to the API implementation
 		 * @param value
 		 * 	The value to set the parameter to. This method is used when setting a parameter to a int type.
 		 */		 
		CFAPI_DLLEXPORTS Request& add(cfapi::RequestParameters parameter, int value);

		/**
 		 * A CSP request message. Should be populated with the desired parameters.
 		 *
 		 * @param parameter 
 		 * 	An integer value corresponding to a CTF id.  The user may specify one of the known parameter values (as defined in RequestParameters) as well as values not defined in the API, but possibly known by upstream components.
 		 * @param value
 		 * 	The value to set the parameter to. This method is used when setting a parameter to an int type.
 		 */
		CFAPI_DLLEXPORTS Request& add(int parameter, int value);
        
		/**
 		 * A CSP request message. Should be populated with the desired parameters
 		 *
 		 * @param parameter
 		 * 	An enumerated value corresponding to one of the RequestParameters known to the API implementation
 		 * @param value
 		 * 	The value to set the parameter to. This method is used when setting a parameter to a int64_t type.
 		 */		 
		CFAPI_DLLEXPORTS Request& add(cfapi::RequestParameters parameter, int64_t value);

		/**
 		 * A CSP request message. Should be populated with the desired parameters.
 		 *
 		 * @param parameter 
 		 * 	An integer value corresponding to a CTF id.  The user may specify one of the known parameter values (as defined in RequestParameters) as well as values not defined in the API, but possibly known by upstream components.
 		 * @param value
 		 * 	The value to set the parameter to. This method is used when setting a parameter to an int64_t type.
 		 */
		CFAPI_DLLEXPORTS Request& add(int parameter, int64_t value);

		/**
		 * A CSP request message. Should be populated with the desired parameters
		 *
		 * @param parameter
		 * 	An enumerated value corresponding to one of the RequestParameters known to the API implementation
		 * @param value
		 * 	The value to set the parameter to. This method is used when setting a parameter to a string type.
		 */
		CFAPI_DLLEXPORTS Request& add(cfapi::RequestParameters parameter, const std::string& value);
        
		/**
 		 * A CSP request message. Should be populated with the desired parameters
 		 *
 		 * @param parameter
 		 * 	An integer value corresponding to a CTF id.  The user may specify one of the known parameter values (as defined in RequestParameters) as well as values not defined in the API, but possibly known by upstream components.
 		 * @param value
 		 * 	The value to set the parameter to. This method is used when setting a parameter to a string type.
 		 */
		CFAPI_DLLEXPORTS Request& add(int parameter, const std::string& value);

		/**
	 	 * Return a pre-generated unique tag for this request prior to calling Session::send().  This call is optional, as Session::send() can generate the unique tag.
 		 */
		CFAPI_DLLEXPORTS int64_t generateTag();


		/**
	 	 * Clear internal state of request object
 		 */
		CFAPI_DLLEXPORTS void clearRequest();

	private:
		Request();
		Request(cfapi_internal::SessionImpl *session);
		~Request();
		cfapi_internal::RequestImpl *requestImpl;
		friend class cfapi_internal::SessionImpl;
		friend class Session;

    };

}
 
#endif
