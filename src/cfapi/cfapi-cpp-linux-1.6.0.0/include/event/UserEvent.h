#ifndef CFAPI_USEREVENT_H
#define CFAPI_USEREVENT_H

#include <string>

#include "dllexport.h"

namespace cfapi_internal
{
	class UserEventImpl;
	class UserInfoImpl;
	class SessionImpl;
}


namespace cfapi
{ 
    class UserInfo;
    class Session;

	/**
	 * UserEvents represent information regarding an authorization request.
	 * UserEvents are returned to the user via their UserEventHandler implementation.
	 * 
	 * @see UserEventHandler
	 * 
	 */
    class UserEvent
    {
	public:
	/**
	 * enumeration of user event types
	 */
        enum Types
        {
            AUTHORIZATION_FAILURE=0,

            AUTHORIZATION_SUCCESS=1
        };

	/**
	 * Returns information about the specific type of event that occurred.
	 */
		CFAPI_DLLEXPORTS  Types getType() const;

	/**
	 * Returns the object associated with this user that this event describes.
	 */
		CFAPI_DLLEXPORTS  UserInfo& getUserInfo() const;

	/**
	 * Returns the session that this event describes
	 */
		CFAPI_DLLEXPORTS  Session& getSession() const;

	/**
	 * Returns the return code for the login event
	 */
		CFAPI_DLLEXPORTS  int getRetCode() const;

	/**
	 * Returns the string description for the return code for the login event
	 */
		CFAPI_DLLEXPORTS  std::string getRetCodeString() const;

	private:
		UserEvent();
		UserEvent(Types type, UserInfo *userInfo, cfapi_internal::SessionImpl *session, int rc);
        	~UserEvent();
		cfapi_internal::UserEventImpl *userEventImpl;
		friend class cfapi_internal::UserInfoImpl;

    };

}

#endif
