#include "api.h"

APIFactoryWrap::APIFactoryWrap(const std::string &appName, const std::string &appVersion,
                               bool debug, const std::string &logFileName, std::string usage,
                               const std::string &username, const std::string &password,
                               const cfapi::UserEventHandler &userHandler,
                               const cfapi::SessionEventHandler &sessionHandler)
{
    ptr = cfapi::APIFactory::getInstance();
    ptr->initialize(appName, appVersion, debug, logFileName, usage);
    primaryUser = &ptr->createUserInfo(username, password, const_cast<cfapi::UserEventHandler &>(userHandler));
    session = &ptr->createSession(*primaryUser, const_cast<cfapi::SessionEventHandler &>(sessionHandler));
    printf("api init done.\n");
}

APIFactoryWrap::~APIFactoryWrap()
{
    cfapi::APIFactory::getInstance()->destroySession(*session);
    cfapi::APIFactory::getInstance()->destroyUserInfo(*primaryUser);
    cfapi::APIFactory::getInstance()->uninitialize();
};