#ifndef CFAPI_USERINFO_H
#define CFAPI_USERINFO_H

#include "event/UserEventHandler.h"

#include "dllexport.h"

namespace cfapi_internal
{
	class SessionImpl;
	class UserInfoImpl;
	class CspClient;
}

namespace cfapi
{
	/**
	 * Object that is associated with a particular user.
	 * This object tracks whether the user is authenticated and is used in conjunction with any content entitlement.
	 * 
	 * @see UserEventHandler
	 * 
	 */
    class UserInfo
    {
    public:
	
	/**
	 * State of this user.
	 */
        enum States
        {
		/**
		*Not authenticated to any backend CSPs
		*/
		NOT_AUTHENTICATED=0,
		/**
		*Authenticated to all backend CSPs
		*/
		AUTHENTICATED=1,
		/**
		*Authenticated to at least one backend CSP, but not authenticated to at least one CSP
		*/
		PARTIALLY_AUTHENTICATED=2
        };

	/**
	 * Get state of this user.
	 */
		CFAPI_DLLEXPORTS States getState();

    private:
	UserInfo();
	UserInfo(const std::string &username, const std::string &password, cfapi::UserEventHandler &userHandler);
        ~UserInfo();	
	cfapi_internal::UserInfoImpl *userInfoImpl;
	friend class cfapi_internal::SessionImpl;
	friend class cfapi_internal::CspClient;
	friend class APIFactory;
    };

}

#endif
