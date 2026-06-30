
#ifndef CFAPI_APIFACTORY_H
#define CFAPI_APIFACTORY_H

#include <string>
#include "event/UserEventHandler.h"
#include "user/UserInfo.h"
#include "session/Session.h"
#include "event/SessionEventHandler.h"

#include "dllexport.h"

namespace cfapi
{

/**
 * This class acts as a factory for the CF API 
 */
    class APIFactory
    {
    public:

	/**
 	 * Gets an instance of APIFactory object
 	 */
        static CFAPI_DLLEXPORTS APIFactory* getInstance();

	/**
 	 * Create a new session.
 	 */
		CFAPI_DLLEXPORTS Session& createSession(UserInfo& primaryUser, SessionEventHandler& sessionHandler);

	/**
 	 * Destroy the session after the user is done with it.
 	 */
		CFAPI_DLLEXPORTS void destroySession(Session& session);

	/**
 	 * Create a new UserInfo object.
 	 */
		CFAPI_DLLEXPORTS UserInfo& createUserInfo( const std::string& username, const std::string& password, UserEventHandler& userHandler);

	/**
 	 * Destroy the UserInfo after the user is done with it.  If passed into createSession(), do not call until after destroySession().
 	 */
		CFAPI_DLLEXPORTS void destroyUserInfo(UserInfo& userinfo);


	/**
 	 * Initializes the API for real use, must be called before any request
 	 * Parameter "usage" is used for only internal purpose;
 	 */
		CFAPI_DLLEXPORTS int initialize(const std::string& appName, const std::string& appVersion, bool debug, const std::string& logFileName, std::string usage ="External");

        /**
 	 * Un-initializes the API after the user is done with it.
 	 */
		CFAPI_DLLEXPORTS int uninitialize();

    private:
        APIFactory() {}
        APIFactory(const APIFactory&);
        APIFactory& operator=(const APIFactory&); 

        static APIFactory* instance;
        static int referenceCount;
    };

}

#endif
