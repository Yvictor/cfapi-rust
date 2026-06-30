#ifndef CFAPI_SESSION_H
#define CFAPI_SESSION_H 1

#include "ConnectionConfig.h"
#include "SessionConfig.h"
#include "../request/Request.h"
#include "../event/MessageEventHandler.h"
#include "event/SessionEventHandler.h"
#include "event/StatisticsEventHandler.h"
#include "user/UserInfo.h"

#include "dllexport.h"


namespace cfapi_internal
{
	class SessionImpl;
	class SessionEventImpl;
}


namespace cfapi
{
class APIFactory;
/**
 * The Session is the main entry point for applications. This manages connection life cycles, as well as inbound and outbound
 * content. Applications will typically create a Session, then specify any non-default configuration. At
 * this point, the Session.start() method will begin running the session. 
 * 
 * Once the session is established (indicated by SessionEvent.Types.SESSION_ESTABLISHED), user(s) can be authenticated with the
 * session. An authenticated user can create Streaming and/or Snapshot requests, which can be opened on the session. As content is
 * received on these open requests, it will be put on the specified event queue or the users handler function will be directly
 * invoked.
 * 
 */
class Session
{

	public:
		
		/**
		* session states
		*/
		enum States
		{
			/**
			* The session is unavailable; none of the connections represented by the session are up
			*/
			CFAPI_SESSION_UNAVAILABLE=0,

			/**
			* The session is established; the connections represented by the session are up and no recovery is needed
			*/
			CFAPI_SESSION_ESTABLISHED=1,

			/**
			* The session is in recovery; at least one of the connections within the session is undergoing the recovery process
			*/
			CFAPI_SESSION_RECOVERY=2
		};

		/**
		* Returns the state of this Session.
		* 
		* @return This indicates whether the user is authenticated or not and is actively receiving data.
		*/
		CFAPI_DLLEXPORTS States getState();


		/* Configuring Session */

		/* this will evolve to support the configuration we need */
		// SessionConfig
		// controls API specific behavior for managing and running session
		/**
		* Provides session configuration reference to the user, allowing them to configure or query session configuration information
		* Session Configuration controls local behaviors within the API
		* 
		* @return SessionConfig getSessionConfig();
		*/
		CFAPI_DLLEXPORTS SessionConfig & getSessionConfig();

		/**
		* Provides connection configuration reference to the user, allowing them to configure or query connection information
		* associated with the Session Connection Configuration configures the connection and any parameters that are sent to the
		* server
		* 
		* @return Connection Configuration object
		*/
		CFAPI_DLLEXPORTS ConnectionConfig & getConnectionConfig();

		/**
		* Provides user reference to the user, allowing them to query authentication state
		* 
		* @return UserInfo reference
		*/
		CFAPI_DLLEXPORTS UserInfo& getUserInfo();

		/* Session startup and shutdown */
		/**
		* Starts this session. Typically invoked after configuration is completed, but will rely on default configuration if no
		* configuration specified
		* 
		* @return true if session has been or is already started; false if fails
		*/
		CFAPI_DLLEXPORTS bool start();

                /**
                * Starts this session. Typically invoked after configuration is completed, but will rely on default configuration if no
                * configuration specified
                * 
                * @param failReason
                *            String to be updated with a failure reason 
                * @return true if session has been or is already started; false if fails
                */
		CFAPI_DLLEXPORTS bool start(std::string &failReason);

		/**
		* Stops this session. Typically invoked when application wishes to shut down.
		* 
		* @return true if session has been or is already stopped
		*/
		CFAPI_DLLEXPORTS bool stop();


		/* Creating Requests */

		/**
		* Creates an empty Request, which can be populated by the user to send a request to the CSP(s)
		* 
		* @return Request object when successful, NULL when failure occurs.
		*/
		CFAPI_DLLEXPORTS Request & createRequest();

		/**
 		 * Free request object when no longer needed
 	 	 */
		CFAPI_DLLEXPORTS void freeRequest(Request &req);


		/* Opening Content */

		/**
		* Issues a request to the CSP(s). The associated response will be dispatched directly to the specified event handler as the data
		* is received. When successful, this should result in one MessageEvent per message being delivered.  Note that due to the 
		* asynchronous and multithreaded nature of the API, it is theoretically possible that a MessageEvent could be returned for this 
		* request before the send() completes.  If you need the handle/tag before the first MessageEvent for this request, use
		* Request::generateTag().
		* 
		* @param req
		*            Request indicating identifying information for desired content
		* @return handle associated with this request (the auto-generated 5026/QUERY.TAG for this request)
		* 	If the Session's input queue is full (non-watchlist mode), send will fail and return a tag of 0.
		*/
		CFAPI_DLLEXPORTS int64_t send(Request &req);

		/**
		* Register a MessageEventHandler to be called whenever data is received for all requests.
		* 
		* @param responseEH
		*            implemented MessageEventHandler function which will be invoked when content becomes available
		*/
		CFAPI_DLLEXPORTS void registerMessageEventHandler(MessageEventHandler *responseEH);

		/**
		* Register a StatisticsEventHandler to be called periodically when new statistics are published.
		* 
		* @param statsEH
		*            implemented StatisticsEventHandler function which will be invoked when statistics becomes available
		* @param interval
		*            interval in seconds to report statistics 
		*/
		CFAPI_DLLEXPORTS void registerStatisticsEventHandler(StatisticsEventHandler *statsEH, int interval);

	private:
		Session();
		Session(UserInfo& primaryUser, cfapi::SessionEventHandler& sessionHandler, bool isValid);
		~Session();
		friend class APIFactory;
		friend class cfapi_internal::SessionEventImpl;
		cfapi_internal::SessionImpl *sessionImpl;



}; //class
} // namespace cfapi

#endif // CFAPI_SESSION_H
