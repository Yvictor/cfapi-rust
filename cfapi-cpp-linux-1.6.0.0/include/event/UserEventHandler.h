#ifndef CFAPI_USEREVENTHANDLER_H
#define CFAPI_USEREVENTHANDLER_H

#include "UserEvent.h"

namespace cfapi
{

/**
 * User implements this class and provides handling functionality for user events.
 * User events communicate information about authentication within a session and services that are available to the user within the session.
 */
    class UserEventHandler
    {
    public:
        virtual ~UserEventHandler() {}

	/**
	* 
	* @param userEvent
	*            the UserEvent to process
	*/
        virtual void onUserEvent(const UserEvent& userEvent) = 0;
            
    };

}

#endif